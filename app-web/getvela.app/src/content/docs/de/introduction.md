---
title: Einführung
description: Was Vela ist, für wen es gebaut wurde und welche Idee hinter einer selbstverwahrten Smart Wallet ohne Seed-Phrase steckt.
---

# Einführung

Vela ist eine **selbstverwahrte Smart Wallet** für EVM-Netzwerke. Die Schlüssel
gehören dir, aber es gibt keine Seed-Phrase zum Aufschreiben — du signierst mit
einem Passkey, per Gesicht oder Fingerabdruck.

Diese Dokumentation zeigt, wie du anfängst, eine Wallet erstellst, Token bewegst
und das Sicherheitsmodell dahinter verstehst.

## Die kurze Fassung

- **Selbstverwahrt.** Über dein Geld bestimmt ein Schlüssel, den nur du benutzen
  kannst. Vela (die Firma) kann dein Geld weder bewegen noch einfrieren noch
  zurückholen.
- **Keine Seed-Phrase.** Dein Signaturschlüssel ist ein Passkey in der sicheren
  Hardware deines Geräts. Es gibt keine zwölf Wörter, die verloren gehen oder
  abgefischt werden können.
- **Ein Safe Smart Account.** Jede Wallet ist ein
  [Safe](https://github.com/safe-fndn/safe-smart-account)-Vertrag, betrieben über
  ERC-4337 Account Abstraction — genau das erlaubt dir, mit einem Passkey zu
  signieren und jede Transaktion vor der Freigabe zu lesen.
- **12 Netzwerke, eine Adresse.** Ethereum, BNB Chain, Polygon, Arbitrum,
  Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad und World Chain —
  dazu eigene Netzwerke — alle unter derselben Adresse.
- **Lesbares Signieren.** Wo ein Deskriptor existiert, wird die Transaktion in
  eine verständliche Absicht übersetzt (ERC-7730). Wo nicht, fällt Vela auf eine
  Best-Effort-Dekodierung zurück und warnt dich. Aufrufe, die es nicht lesen
  kann, werden markiert, nicht versteckt.
- **Open Source.** Wallet und alle Dienste sind
  [öffentlich auf GitHub](https://github.com/mondaylabsltd/vela-wallet), damit
  jeder prüfen kann, was sie tun.
- **Alpha-Software.** Vela funktioniert und hält echtes Geld, aber es hat noch
  keine Jahre im Produktivbetrieb hinter sich. Fang mit kleinen Beträgen an. Der
  [Alpha-Beitrag](/blog/vela-is-in-alpha) erklärt, was das bedeutet.

## Für wen es gebaut ist

Vela ist für Leute, die echte Selbstverwahrung wollen, ohne die Fußangel der
Seed-Phrase-Verwaltung — und für die, die damit schon einmal auf die Nase
gefallen sind. Wenn du dein Handy entsperren kannst, kannst du Vela benutzen.

## Wie es weitergeht

- [Vela installieren](/de/docs/install) — läuft im Browser, nichts herunterzuladen.
- [Wallet erstellen](/de/docs/create-wallet) — deine erste Wallet in etwa einer Minute.
- [So funktionieren Passkeys](/de/docs/passkeys) — das Sicherheitsmodell, klar erklärt.
- [Whitepaper](/de/docs/whitepaper) — die vollständige Architektur und das Vertrauensmodell.

Wenn dich das *Warum* mehr interessiert als das *Wie*, erzählt der
[Blog](/blog), wie Vela entsteht.
