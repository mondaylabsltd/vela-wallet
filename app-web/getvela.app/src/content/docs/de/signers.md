---
title: Signaturschlüssel und Sicherheitsschlüssel
description: Eine Vela-Wallet kann bis zu sieben Signaturschlüssel haben — Passkeys, ein Gerät in der Nähe oder einen Sicherheitsschlüssel der YubiKey-Klasse — und jeder einzelne davon signiert. Sie werden beim Erstellen der Wallet festgelegt, und diese Seite erklärt, warum das keine vergessene Einschränkung ist.
---

# Signaturschlüssel und Sicherheitsschlüssel

Eine Vela-Wallet ist ein Safe, und ein Safe hat Eigentümer. Deiner kann **bis zu
sieben** haben, mit einem Schwellenwert von **eins**: Jeder einzelne
Signaturschlüssel kann eine Transaktion allein autorisieren. Geschrieben als
`1-of-n`.

## Was ein Signaturschlüssel sein kann

Drei Arten, frei kombinierbar:

| Methode | Was es ist | Typisches Beispiel |
| --- | --- | --- |
| **Plattform** | Der eingebaute Authenticator des Geräts, das du gerade benutzt | Face ID / Touch ID auf diesem Handy oder Laptop, synchronisiert über iCloud-Schlüsselbund oder Google Passwortmanager |
| **Gerät in der Nähe** | Ein anderes Gerät, das du per Code-Scan erreichst | Dein Handy signiert für deinen Desktop, über den WebAuthn-Hybrid-Transport |
| **Sicherheitsschlüssel** | Ein abnehmbarer Authenticator über USB oder NFC | YubiKey und andere FIDO2-Schlüssel |

Alle drei sind WebAuthn-Credentials auf der **P-256**-Kurve. Für den Safe sind sie
nicht zu unterscheiden: Jeder ist ein Eigentümer, dessen Signatur der
On-Chain-WebAuthn-Prüfer auf dieselbe Weise verifiziert.

Ein Sicherheitsschlüssel kann dein **erster** Signaturschlüssel sein, nicht nur
ein Backup. Wenn deine Wallet nie von einem Apple- oder Google-Konto abhängen
soll, ist das die Einstellung, die genau das erreicht: beim Erstellen einen
YubiKey registrieren und damit signieren.

## Warum sie beim Erstellen festgelegt werden

Das überrascht viele, deshalb hier der Mechanismus statt einer Entschuldigung.

Deine Wallet-Adresse wird aus der Menge ihrer Eigentümer **abgeleitet**. Vela
berechnet sie mit `CREATE2` aus den Safe-Setup-Daten — die den öffentlichen
Schlüssel jedes Signaturschlüssels enthalten — bevor on-chain irgendetwas
existiert. Genau deshalb kannst du an einer Adresse empfangen, die es noch nicht
gibt.

Die Folge ist Arithmetik, keine Policy: **Eine andere Schlüsselmenge ist eine
andere Adresse.** Einen achten Signaturschlüssel später hinzuzufügen erweitert
deine Wallet nicht; es berechnet eine neue Wallet, unter einer neuen Adresse, mit
keinem Cent deines Geldes darin.

Die Frage „Kann ich später einen Schlüssel hinzufügen?“ hat also zwei ehrliche
Antworten:

- **Bevor du sie befüllst:** ja — die Adresse hat sich auf nichts festgelegt,
  erstelle die Wallet einfach neu mit den Schlüsseln, die du willst.
- **Nachdem du sie befüllt hast:** Die Adresse ist der Ort, an dem dein Geld
  liegt. Eigentümer eines aufgesetzten Safe zu ändern ist eine Safe-Operation,
  die Vela derzeit nicht anbietet. Plane die Schlüsselmenge beim Erstellen.

## Wovor dich das tatsächlich schützt

**Ein verlorenes Gerät.** Mit mehr als einem Signaturschlüssel ist ein verlorenes
Handy eine Unannehmlichkeit: Ein anderer Schlüssel signiert. Mit genau einem
Schlüssel und abgeschalteter Betriebssystem-Synchronisation ist ein verlorenes
Handy eine verlorene Wallet — deshalb ist „dein Passkey synchronisiert sich
automatisch“ die Beschreibung einer Einstellung, die du kontrollierst, und keine
Garantie, die wir für dich geben könnten.

**Ein Plattformkonto, dem du nicht mehr traust.** Liegt dein Passkey im
iCloud-Schlüsselbund oder im Google Passwortmanager, kann ihn potenziell nutzen,
wer dieses Konto kontrolliert. Einen Sicherheitsschlüssel hältst du selbst, und er
synchronisiert sich nirgendwohin.

Und wovor es dich **nicht** schützt, weil `1-of-n` in beide Richtungen schneidet:
Ein zweiter Schlüssel ist ein zweiter Weg *hinein*, kein zweites Schloss. Wer
irgendeinen deiner Signaturschlüssel erlangt, kann allein signieren. Mehr
Schlüssel heißt mehr Widerstandsfähigkeit gegen Verlust und mehr Angriffsfläche
gegen Diebstahl; das ist der Handel, und du triffst die Wahl.

## Wiederherstellen ist nicht Hinzufügen

Das sind zwei verschiedene Dinge, und die Doku hält sie auseinander:

- [Wiederherstellung und Anmeldung](/de/docs/recovery) — mit einem Schlüssel, den
  du schon hast, auf einem neuen Gerät zu einer bestehenden Wallet zurückkommen.
- Diese Seite — vorab entscheiden, welche Schlüssel überhaupt existieren.
