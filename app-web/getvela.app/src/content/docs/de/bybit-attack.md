---
title: Der Bybit-Angriff
description: Im Februar 2025 verlor Bybit rund 1,5 Milliarden Dollar. Die Safe-Verträge waren nicht kaputt — die Oberfläche war es. Diese Seite erklärt den Weg und was in Velas Design ihn schließt.
---

# Der Bybit-Angriff

Am 21. Februar 2025 verlor Bybit rund **1,5 Milliarden Dollar** aus einer
Safe-Multisig-Cold-Wallet. Es ist der größte Diebstahl in der Geschichte dieser
Branche, und er lohnt eine genaue Lektüre, weil fast alles daran *korrekt* war —
bis auf eine Sache.

## Was passiert ist

Die Kurzfassung aus den öffentlichen Post-mortems:

1. Ein Angreifer kompromittierte den **Rechner eines `Safe{Wallet}`-Entwicklers**
   und schleuste bösartiges JavaScript in den AWS-S3-Bucket ein, der das
   `Safe{Wallet}`-Frontend auslieferte. Der Code ging am 19. Februar hinein und
   wurde am 21. Februar ausgelöst, zielgerichtet auf Bybits konkreten Safe.
2. Bybits Unterzeichner öffneten die Oberfläche und prüften eine Transaktion, die
   gewöhnlich aussah.
3. Was tatsächlich an ihre **Hardware-Wallets** ging, war nicht diese Transaktion.
   Es war ein `delegatecall`, der die `masterCopy` des Safe-Proxys überschrieb —
   Slot 0 — und die gesamte Account-Implementierung durch die des Angreifers
   ersetzte.
4. Die Unterzeichner gaben frei. Die Signaturen waren gültig. Der Vertrag tat
   genau das, was ihm gesagt wurde.

Die Zuordnung wurde öffentlich mit nordkoreanisch verbundener Aktivität gemacht
(das FBI nannte den TraderTraitor-Cluster).

## Was *nicht* kaputt war

- **Nicht die Safe-Verträge.** Sie führten eine gültig signierte Anweisung aus.
  Kein Bug in Safe wurde ausgenutzt.
- **Nicht die Kryptografie.** Jede Signatur war echt.
- **Nicht die Hardware-Wallets.** Ledger-Geräte waren im Ablauf und signierten
  trotzdem — weil eine Hardware-Wallet zeigt, was man ihr gibt, und was man ihr
  gab, war die bösartige Payload. Ein Gerät, das einen `delegatecall` nicht in
  etwas übersetzen kann, das ein Mensch beurteilen kann, schützt den
  *Schlüssel*, nicht die *Entscheidung*.

Kaputt war die Annahme unter jeder Wallet-Oberfläche: **dass der Bildschirm, der
eine Transaktion beschreibt, und die Bytes, die signiert werden, dasselbe sind.**

## Warum das der Normalfall ist und kein Ausreißer

Jede Signatur, die du je in einer Web-Wallet erzeugt hast, ruhte auf dieser
Annahme. Die Oberfläche baut die Payload, die Oberfläche zeichnet die
Zusammenfassung, und nichts Unabhängiges prüft, ob beides zusammenpasst. Wird der
Code, der diese Oberfläche ausliefert, ersetzt — durch eine kompromittierte
Build-Pipeline, ein gekapertes CDN, eine bösartige Abhängigkeit, ein gestohlenes
Deploy-Credential — wird die Zusammenfassung zu dem, was der Angreifer will, und
deine Signatur ist echt.

Das ist das Risiko, auf das Velas Signaturdesign zielt. Nicht Phishing. Kein
geleakter Schlüssel. **Ein Signaturbildschirm, der dich anlügt.**

## Was Vela dagegen tut

**Klartext-Signatur, bis zur Calldata.** Jede Transaktion wird vor der Freigabe in
lesbare Absicht übersetzt — Betrag, Empfänger, was der Aufruf tatsächlich tut
([ERC-7730](/de/docs/clear-signing)). Ein Aufruf, den wir nicht dekodieren können,
wird **als nicht dekodierbar markiert**, nicht stillschweigend so dargestellt, als
wäre er in Ordnung. Bybits Payload war ein `delegatecall`, der eine
Implementierungsadresse austauschte; genau so etwas muss einen Unterzeichner
abrupt stoppen — und dass es hinter einer freundlichen Zusammenfassung steckte,
ist der Grund, warum es das nicht tat.

**Ein unabhängiger Pfad, der die Oberfläche prüfen kann.** Vela baut eine
Signaturseite ohne Build-Schritt und ohne Abhängigkeiten, die die Absicht selbst
darstellt und die WebAuthn-Signatur selbst durchführt — ein einzelner Ordner
statischer Dateien, den du von vorn bis hinten lesen, selbst ausliefern oder als
Browser-Erweiterung betreiben kannst. Sein ganzer Zweck ist es, eine zweite
Meinung zu sein, die nicht die Lieferkette der Haupt-App teilt. *Stand: gebaut und
getestet, noch nicht ausgerollt.* Wenn sie kommt, ist sie optional, und diese
Seite sagt es klar, sobald sich das ändert.

**Kein Vertrag, den wir upgraden können.** Bybits Payload funktionierte, indem sie
die Implementierung des Accounts ersetzte. Velas Accounts sind
[unveränderte Safe v1.4.1](/de/docs/account-contract), und Vela hält auf ihnen
keine privilegierte Rolle — keinen Admin-Schlüssel, keinen Upgrade-Pfad, zu dessen
Nutzung man uns zwingen oder kompromittieren könnte.

**Eine frische biometrische Prüfung für jede Signatur.** Es gibt keinen
langlebigen Session-Key, also auch kein Zeitfenster, in dem etwas ohne dich in
deinem Namen signieren kann.

**Selbst hosten als Rückfallebene.** Die App und jeder Backend-Dienst sind Open
Source. Willst du unserer Build-Pipeline überhaupt nicht vertrauen, betreib deine
eigene — das ist die einzige Antwort auf diese Angriffsklasse, die kein Vertrauen
in jemanden verlangt.

## Was Vela nicht behauptet

Velas Frontend könnte auf dieselbe Weise kompromittiert werden wie das von
`Safe{Wallet}`. Unser Code ist nicht auditiert. Etwas anderes zu behaupten wäre
genau die Art von Zusicherung, mit der dieser Vorfall hätte Schluss machen sollen.

Was das Design versucht, ist, den Weg zu verengen: die Payload lesbar statt
undurchsichtig zu machen, das Upgrade-Primitiv zu entfernen, auf das der Angriff
baute, und dir eine Möglichkeit zu geben, mit etwas zu prüfen, das nicht wir sind.
Die ehrliche Zusammenfassung lautet: **Diese Angriffsklasse wird durch das Design
abgeschwächt, nicht beseitigt** — und die Teile, die sie weiter härten würden,
stehen unfertig in
[Audits und bekannte Probleme](/de/docs/security-audits).

## Quellen

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
