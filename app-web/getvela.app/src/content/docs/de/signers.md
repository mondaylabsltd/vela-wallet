---
title: Signaturschlüssel und Sicherheitsschlüssel
description: "Eine Vela-Wallet kann bis zu sieben Signaturschlüssel haben – Passkeys, ein Handy in der Nähe oder einen Sicherheitsschlüssel wie den YubiKey –, und jeder einzelne kann signieren. Sie werden beim Erstellen der Wallet gewählt; diese Seite erklärt, warum, und was zu tun ist, wenn ein Schlüssel kompromittiert sein könnte."
source: 072891bd4768
---

# Signaturschlüssel und Sicherheitsschlüssel

Eine Vela-Wallet ist ein Safe, und ein Safe hat Eigentümer. Dein Safe kann **bis zu
sieben** haben, mit einem Schwellenwert von **eins**: Jeder einzelne
Signaturschlüssel kann eine Transaktion allein autorisieren. Man schreibt das
`1-of-n`.

## Was ein Signaturschlüssel sein kann

Drei Arten, frei kombinierbar:

| Methode | Was es ist | Typisches Beispiel |
| --- | --- | --- |
| **Plattform** | Der Authentifikator, der in das Gerät eingebaut ist, das du gerade nutzt | Face ID / Touch ID auf diesem Handy oder Laptop, synchronisiert über iCloud-Schlüsselbund oder Google Passwortmanager |
| **Gerät in der Nähe** | Ein anderes Gerät, das du per Code-Scan erreichst | Dein Handy signiert für deinen Desktop, über den Hybrid-Transport von WebAuthn |
| **Sicherheitsschlüssel** | Ein abnehmbarer Authentifikator per USB oder NFC | YubiKey und andere FIDO2-Schlüssel |

Alle drei sind WebAuthn-Anmeldedaten auf der Kurve **P-256**. Für das Safe sind sie
nicht zu unterscheiden: Jeder ist ein Eigentümer, dessen Signatur das Passkey-Modul
von Safe on-chain auf dieselbe Weise prüft. (Der erste Schlüssel wird vom gemeinsam
genutzten Signer von Safe geprüft, dem Shared Signer; jeder weitere von einem eigenen
kleinen Signer-Vertrag, den die Factory von Safe erstellt, wenn die Wallet zum ersten
Mal auf einer Chain bereitgestellt wird.)

Welche Arten jede App nutzen kann:

| App | Dieses Gerät | Handy in der Nähe (QR) | Sicherheitsschlüssel |
| --- | --- | --- | --- |
| Web-Wallet, Browser-Erweiterung | Ja | Ja | USB oder NFC, über den Browser |
| Desktop (macOS, Windows, Linux) | macOS und Windows | Ja | USB |
| Android | Ja (mit Google-Play-Diensten) | Ja | USB |
| iOS | Ja | Ja | YubiKey mit USB-C oder Lightning, Firmware 5.8 oder neuer |

Die Option „Dieses Gerät“ der Desktop-App (Touch ID, Windows Hello) und ihre
Windows-Unterstützung insgesamt sind neu und weniger getestet als die anderen Wege;
ein Handy oder ein Sicherheitsschlüssel ist dort heute die verlässliche Wahl.

Ein Sicherheitsschlüssel kann dein **erster** Signaturschlüssel sein, nicht nur ein
Backup. Soll deine Wallet gar nicht von einem Apple- oder Google-Konto abhängen,
erstelle sie mit **zwei** Sicherheitsschlüsseln und bewahre einen davon an einem
sicheren Ort auf. (Eine Wallet, deren einziger Schlüssel nirgends synchronisiert wird,
lässt sich nicht erstellen: Die App verlangt einen zweiten Schlüssel, denn mit diesem
einen Gerät wäre auch die Wallet verloren.)

## Warum sie beim Erstellen gewählt werden

Das überrascht viele, deshalb hier der Mechanismus statt einer Entschuldigung.

Deine Wallet-Adresse wird aus ihrer Eigentümermenge **abgeleitet**. Vela berechnet sie
mit `CREATE2` aus den Setup-Daten des Safe – die den öffentlichen Schlüssel jedes
Signaturschlüssels enthalten –, bevor irgendetwas on-chain bereitgestellt ist. Genau
das erlaubt dir, Geld an einer Adresse zu empfangen, die es noch gar nicht gibt.

Für die Adresse ist die Folge reine Arithmetik: **Ein anderer Schlüsselsatz ist eine
andere Adresse.** Einen weiteren Signaturschlüssel später hinzuzufügen würde deine
Wallet nicht erweitern; es würde eine neue Wallet berechnen, an einer neuen Adresse,
ohne dein Geld darin.

Die Frage „Kann ich später einen Schlüssel hinzufügen?“ hat deshalb zwei ehrliche
Antworten:

- **Bevor du Geld einzahlst**: ja – die Adresse ist noch an nichts gebunden, also
  erstelle die Wallet einfach neu mit den Schlüsseln, die du willst.
- **Nachdem du Geld eingezahlt hast**: An dieser Adresse liegt dein Geld. Safe selbst
  kann Eigentümer auf einer Chain ändern, auf der deine Wallet bereits bereitgestellt
  ist – aber auf jeder Chain, auf der sie es noch nicht ist, steht dieselbe Adresse
  weiterhin für die ursprünglichen Schlüssel, und die Eigentümer würden von Chain zu
  Chain auseinanderlaufen. Sie über Chains hinweg synchron zu halten ist möglich –
  manche Smart Wallets tun das –, aber Vela hat es nicht gebaut und bietet deshalb
  keine Eigentümerwechsel an. Plane den Schlüsselsatz beim Erstellen.

## Wovor dich das tatsächlich schützt

**Ein verlorenes Gerät.** Mit mehr als einem Signaturschlüssel ist ein verlorenes
Handy lästig: Ein anderer Schlüssel signiert. Mit genau einem Signaturschlüssel und
ausgeschalteter Synchronisierung ist ein verlorenes Handy eine verlorene Wallet –
deshalb beschreibt „dein Passkey wird automatisch synchronisiert“ eine Einstellung,
die du kontrollierst, und keine Garantie, die wir für dich abgeben können.

**Ein Plattformkonto, dem du nicht mehr vertraust.** Liegt dein Passkey im
iCloud-Schlüsselbund oder im Google Passwortmanager, kann ihn möglicherweise nutzen,
wer dieses Konto kontrolliert. Einen Sicherheitsschlüssel hast du selbst in der Hand,
und er wird nirgends synchronisiert.

Und wovor es dich **nicht** schützt, denn `1-of-n` ist ein zweischneidiges Schwert:
Ein zweiter Schlüssel ist ein zweiter Weg *hinein*, kein zweites Schloss. Wer einen
beliebigen deiner Signaturschlüssel in die Hände bekommt, kann allein signieren. Mehr
Schlüssel bedeuten mehr Widerstandskraft gegen Verlust und mehr Angriffsfläche für
Diebstahl; das ist der Tausch, und die Entscheidung liegt bei dir.

## Wenn ein Schlüssel kompromittiert sein könnte

Ein Schlüssel lässt sich nicht entfernen. Könnte einer deiner Schlüssel in fremden
Händen sein – ein entsperrtes Handy, das verschwunden ist, ein Code, den jemand
gesehen hat, ein Apple- oder Google-Konto, das du nicht mehr kontrollierst –, **zieh
alles in eine neue Wallet um**, erstellt mit Schlüsseln, denen du vertraust. Über die
alte Adresse kann dieser Schlüssel in jedem Netzwerk weiter verfügen – auch über Geld,
das später noch jemand dorthin schickt.

## Wiederherstellen und Hinzufügen

Das sind zwei verschiedene Dinge, und die Dokumentation hält sie auseinander:

- [Wiederherstellung und Anmeldung](/de/docs/recovery) – mit einem Schlüssel, den du
  schon hast, auf einem neuen Gerät zu einer bestehenden Wallet zurückkommen.
- Diese Seite – vorab entscheiden, welche Schlüssel es überhaupt gibt.
