---
title: Der Account-Vertrag
description: Deine Vela-Wallet ist ein unveränderter Safe v1.4.1. Nichts im Vertragspfad stammt von uns — hier steht, was dir das bringt und was es kostet.
---

# Der Account-Vertrag

Deine Wallet ist keine private Datenstruktur einer App. Sie ist ein **Safe
v1.4.1** Smart Account — derselbe Vertrag, der Vermögen hält, das weit größer ist
als alles, was Vela je sehen wird — genau so aufgesetzt, wie Safe ihn
veröffentlicht, ohne jede Änderung.

Der Satz ist kurz, die Folgen sind es nicht, deshalb schreibt diese Seite sie aus.

## Nichts in diesem Pfad ist von uns

Zwischen dir und deinem Geld stehen vier Verträge. Vela hat keinen davon
geschrieben:

| Vertrag | Wer ihn geschrieben hat |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (der Account selbst, ein Proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (prüft deinen P-256-Schlüssel) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | Die ERC-4337-Autoren |

Es gibt keinen Vela-Vertrag. Das Repository enthält überhaupt kein Solidity — mit
einem Befehl nachprüfbar:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # gibt nichts aus
```

Wenn Vela ein Netzwerk hinzufügt, setzt es **jene** Verträge an ihren kanonischen
Adressen auf. Es setzt keinen selbst entworfenen Vertrag auf und hält auf deinem
keine privilegierte Rolle: kein Admin-Schlüssel, kein Upgrade-Pfad, kein Modul,
das wir nachträglich einhängen könnten.

## Warum „unverändert“ das entscheidende Wort ist

Viele Wallets bauen auf „einem Safe“, „einem Safe-Fork“ oder „einem
Safe-inspirierten Account“ auf. Ein Fork ist ein neuer Vertrag mit altem Ruf. Die
Unterschiede, praktisch gesehen:

**Die Audits gelten dem, was du tatsächlich benutzt.** Safes Audit-Berichte
decken den Bytecode genau dieser Releases ab. Die Audits eines Forks decken den
Code vor dem Fork ab. Hat eine Wallet den Account-Vertrag verändert, ist jedes
zitierte Audit das Audit von etwas anderem — und die Änderung ist genau der Teil,
den niemand geprüft hat.

**Das Ökosystem behandelt deinen Account als Safe, weil er einer ist.**
Block-Explorer dekodieren ihn. Safes eigenes Transaktions-Tooling versteht ihn.
Wäre Vela morgen weg, ist deine Wallet kein verwaistes Format — sie ist der Smart
Account mit der breitesten Tool-Unterstützung auf Ethereum, und jede
Safe-kompatible Oberfläche kann sie bedienen. Genau das macht
[„Wenn Vela verschwindet, verschwindet deine Wallet nicht“](/de/docs/why-vela)
zu einer Aussage über Verträge statt über unsere Absichten.

**Die Angriffsfläche ist eine, auf die alle anderen auch schauen.** Ein
maßgeschneiderter Account-Vertrag wird nur von seinem Autor beobachtet. Diesen hier
beobachten alle, die Geld in einem Safe halten.

## Was es kostet

Standard zu sein ist nicht umsonst, und die Abwägungen sind echt:

- **Gas.** Ein Smart Account prüft eine Signatur on-chain. Rechne je nach Chain
  mit etwa dem 1,5- bis 3-Fachen des Gases einer einfachen EOA-Überweisung. Siehe
  [Netzwerke und Gebühren](/de/docs/networks-and-fees).
- **Der Account muss aufgesetzt werden.** Deine Adresse wird mit `CREATE2`
  berechnet, bevor on-chain etwas existiert — du kannst also sofort empfangen,
  aber die erste ausgehende Transaktion bezahlt das Aufsetzen des Vertrags.
- **Nicht jede Chain qualifiziert sich.** Der WebAuthn-Signer prüft eine
  P-256-Signatur on-chain, was die **RIP-7212**-Precompile braucht. Fehlt sie,
  verweigert Vela das Netzwerk, statt auf einen schwächeren Prüfer auszuweichen.
- **Safes Risiko ist jetzt dein Risiko.** Einem weit verbreiteten Vertrag zu
  vertrauen heißt immer noch, einem Vertrag zu vertrauen. Vela kann nur sagen,
  dass es nicht noch eine zweite Sache obendrauf gelegt hat, der du vertrauen
  musst.

## Was auditiert ist und was nicht

Safes Verträge und das WebAuthn-Signer-Modul sind von Dritten auditiert, und die
Berichte sind öffentlich. **Velas eigener App-Code wurde nicht unabhängig
auditiert**, und ein Audit ist nicht terminiert — es ist ein Ziel für den
Zeitpunkt, an dem das Projekt eines finanzieren kann, keine Zusage mit Datum.
Jeder Vertrag, von dem Vela abhängt, sein Audit-Bericht und die Probleme, die wir
verfolgen, stehen in
[Audits und bekannte Probleme](/de/docs/security-audits).

## Überzeug dich selbst

Dein Account liegt on-chain. Öffne ihn in einem Block-Explorer und lies die
Implementierungsadresse: Es wird Safes kanonisches Deployment in v1.4.1 sein,
Byte für Byte, in jedem Netzwerk, das Vela unterstützt.
