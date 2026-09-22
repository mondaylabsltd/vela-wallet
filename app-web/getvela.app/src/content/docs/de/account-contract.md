---
title: Der Account-Vertrag
description: "Deine Vela-Wallet ist ein unverändertes Safe v1.4.1. Kein Vertrag auf dem Weg zu deinem Geld wurde von Vela geschrieben – hier steht genau, welche Verträge es sind, was dir das bringt und was es kostet."
source: 17fbc25a3149
---

# Der Account-Vertrag

Deine Wallet ist keine private Datenstruktur einer App. Sie ist ein
**Safe-v1.4.1**-Smart-Account – der Vertrag, den viele große On-Chain-Treasuries
verwenden –, bereitgestellt genau so, wie Safe ihn veröffentlicht, ohne jede Änderung.

## Nichts auf dem Weg gehört uns

Jeder Vertrag, der dein Geld berühren kann, wurde von Safe oder von den Autoren von
ERC-4337 geschrieben:

| Vertrag | Rolle in deiner Wallet | Geschrieben von |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, über einen Proxy) | Das Konto selbst; Eigentümer, Schwellenwert, Ausführung | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Lässt den EntryPoint das Safe bedienen; zugleich sein Fallback-Handler | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Prüft die P-256-Signaturen des ersten Schlüssels | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) und die Signer, die sie erstellt | Ein kleiner Signer-Vertrag pro zusätzlichem Schlüssel | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Führt deine signierte Operation aus | Autoren von ERC-4337 |

Velas eigene Verträge stehen nicht in dieser Liste: das **Public-Key-Register**, das
die Schlüssel jeder Wallet festhält, damit ein neues Gerät sie finden kann
([Wiederherstellung](/de/docs/recovery)), das zugehörige Domain-Register und der
frühere Index, den sie abgelöst haben. Sie halten kein Geld und haben keine Rolle in
deinem Safe.

Das Wallet-Repository enthält überhaupt kein Solidity – das prüfst du mit einem
einzigen Befehl:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # gibt nichts aus
```

Vela hat keine privilegierte Rolle in deinem Konto: keinen Admin-Schlüssel, keinen
Upgrade-Pfad, kein Modul, das es hinzufügen könnte. Nur deine Schlüssel können dein
Safe ändern.

## Warum „unverändert“ das entscheidende Wort ist

Viele Wallets bauen auf „einem Safe“ auf, auf einem Fork von Safe oder auf einem von
Safe inspirierten Account. Der Unterschied zählt in dreierlei Hinsicht.

**Die Audits gelten für das, was du tatsächlich benutzt.** Die Audits von Safe decken
genau diese Releases ab oder frühere, von denen sie sich nur durch kleine,
dokumentierte Änderungen unterscheiden (Details auf der
[Audit-Seite](/de/docs/security-audits)). Die Audits eines Forks decken den Code vor
dem Fork ab; die Änderung ist der Teil, den niemand auditiert hat.

**Das Ökosystem behandelt dein Konto als Safe, weil es eines ist.** Block-Explorer
dekodieren es, und die Werkzeuge von Safe können es lesen und Transaktionen dafür
bauen. Um diese Transaktionen zu *signieren*, muss ein Programm deinen Schlüssel aber
um eine Signatur für `getvela.app` bitten können, die Domain, zu der deine Passkeys
gehören – deshalb kann die Web-App von Safe, die von einer anderen Domain ausgeliefert
wird, nicht für dich signieren. Die
[Anleitung zum Selbsthosten](/de/docs/self-hosting#if-getvela-app-disappears) listet
auf, womit es geht.

**Die Angriffsfläche beobachten viele andere mit.** Einen maßgeschneiderten
Account-Vertrag beobachtet vor allem sein Autor. Die Kernverträge von Safe beobachtet
jeder, der Geld in einem Safe hat; das 4337- und das Passkey-Modul haben ein kleineres
Publikum, aber ein echtes.

## Was es kostet

Standard zu sein ist nicht umsonst:

- **Gas.** Deine Signatur wird on-chain geprüft, und die Transaktion läuft über den
  EntryPoint. Eine einfache Sendung aus einer bereitgestellten Vela-Wallet verbrauchte
  in unseren Messungen auf Gnosis (September 2026) on-chain etwa 140.000–170.000 Gas;
  eine einfache ETH-Überweisung von einem gewöhnlichen Konto verbraucht 21.000.
  Zusätzlich zum Gas berechnet das Relay seine Gebühr – siehe
  [Netzwerke und Gebühren](/de/docs/networks-and-fees).
- **Das Konto muss bereitgestellt werden.** Deine Adresse wird mit `CREATE2`
  berechnet, bevor irgendetwas on-chain existiert, deshalb kannst du sofort daran
  empfangen; deine erste ausgehende Transaktion in jedem Netzwerk bezahlt die
  Bereitstellung des Vertrags.
- **Nicht jede Chain kommt infrage.** Passkey-Signaturen werden mit dem
  **EIP-7951/RIP-7212**-Precompile geprüft, und seine Adresse ist Teil der Setup-Daten jeder
  Wallet – ein Netzwerk ohne dieses Precompile kann Vela überhaupt nicht ausführen.
- **Das Risiko von Safe ist jetzt dein Risiko.** Einem weit verbreiteten Vertrag zu
  vertrauen heißt immer noch, einem Vertrag zu vertrauen. Vela hat auf dem Weg zu
  deinem Geld keinen zweiten, eigenen Vertrag hinzugefügt, dem du vertrauen müsstest.

## Was auditiert ist und was nicht

Die Verträge von Safe, das 4337- und das Passkey-Modul sowie der EntryPoint v0.7
haben veröffentlichte Audits von Dritten. **Velas eigener Code – die Apps, die
Backend-Dienste und der Registervertrag – hatte kein Audit durch Dritte, und derzeit
ist auch keines angesetzt**; ein Audit ist ein Ziel für den Zeitpunkt, an dem das
Projekt eines finanzieren kann, keine Zusage mit Datum. Jeder Vertrag, sein
Audit-Bericht und die Probleme, die wir verfolgen, stehen unter
[Audits und bekannte Probleme](/de/docs/security-audits).

## Sieh selbst nach

Dein Konto liegt on-chain. Öffne deine Adresse in einem Block-Explorer, sobald die
Wallet bereitgestellt ist: Sie ist ein Safe-Proxy, dessen Implementierung die
kanonische SafeL2-v1.4.1-Bereitstellung von Safe ist, in jedem Netzwerk.

Weiter: [Audits und bekannte Probleme](/de/docs/security-audits).
