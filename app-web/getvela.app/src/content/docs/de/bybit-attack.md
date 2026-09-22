---
title: Der Bybit-Angriff und der Weg, den er nahm
description: "Im Februar 2025 verlor Bybit rund 1,5 Milliarden US-Dollar. Nicht die Safe-Verträge waren kaputt, sondern die Oberfläche. Diese Seite erklärt den Angriffsweg und womit Velas Design ihn schließt."
source: ac56b16b531f
---

# Der Bybit-Angriff und der Weg, den er nahm

Am 21. Februar 2025 verlor Bybit rund **1,5 Milliarden US-Dollar** aus einer
Safe-Multisig-Cold-Wallet. Es ist der größte Diebstahl in der Geschichte der Branche,
und es lohnt sich, ihn genau nachzulesen, weil fast alles daran *korrekt* war – bis auf
eine Sache.

## Was passiert ist

Die Kurzfassung, nach den öffentlichen Post-Mortems:

1. Ein Angreifer kompromittierte den **Rechner eines `Safe{Wallet}`-Entwicklers** und
   schleuste bösartiges JavaScript in den AWS-S3-Bucket ein, der das
   `Safe{Wallet}`-Frontend auslieferte. Der Code wurde am 19. Februar eingeschleust
   und am 21. Februar ausgelöst, gezielt gegen das Safe von Bybit.
2. Bybits Unterzeichner öffneten die Oberfläche und prüften eine Transaktion, die
   gewöhnlich aussah.
3. Was tatsächlich an ihre **Hardware-Wallets** ging, war nicht diese Transaktion. Es
   war ein `delegatecall`, der die `masterCopy` des Safe-Proxys überschrieb – Slot 0 –
   und damit die gesamte Account-Implementierung durch die des Angreifers ersetzte.
4. Die Unterzeichner gaben frei. Die Signaturen waren gültig. Der Vertrag tat genau,
   was ihm gesagt wurde.

Öffentlich zugeordnet wurde der Angriff Aktivitäten mit Verbindung zu Nordkorea (das
FBI nannte den Cluster TraderTraitor).

## Was *nicht* kaputt war

- **Nicht die Safe-Verträge.** Sie führten eine gültig signierte Anweisung aus. Kein
  Fehler in Safe wurde ausgenutzt.
- **Nicht die Kryptografie.** Jede Signatur war echt.
- **Nicht die Hardware-Wallets.** Ledger-Geräte waren beteiligt und signierten
  trotzdem – weil eine Hardware-Wallet zeigt, was man ihr gibt, und was man ihr gab,
  war die bösartige Payload. Ein Gerät, das einen `delegatecall` nicht in etwas
  übersetzen kann, das ein Mensch beurteilen kann, schützt den *Schlüssel*, nicht die
  *Entscheidung*.

Kaputt war die Annahme, die unter jeder Wallet-Oberfläche liegt: **dass der
Bildschirm, der eine Transaktion beschreibt, und die Bytes, die signiert werden,
dasselbe sind.**

## Warum das der Normalfall ist und kein Ausreißer

Die meisten Signaturen, die in einer Web-Wallet entstehen, beruhen auf dieser
Annahme. Die Oberfläche baut die Payload, die Oberfläche stellt die Zusammenfassung
dar, und nichts Unabhängiges prüft, ob beides zusammenpasst. Wird der Code ersetzt,
der diese Oberfläche ausliefert – durch eine kompromittierte Build-Pipeline, ein
gekapertes CDN, eine bösartige Abhängigkeit, gestohlene Zugangsdaten für das
Deployment –, zeigt die Zusammenfassung, was immer der Angreifer will, und deine
Signatur ist echt.

Das ist das Risiko, auf das Velas Signaturdesign zielt. Nicht Phishing. Kein
geleakter Schlüssel. **Ein Signaturbildschirm, der dich anlügt.**

## Was Vela dagegen tut

**Klartext-Signatur, bis in die Calldata.** Jede Transaktion wird vor der Freigabe in
lesbare Absicht übersetzt – Betrag, Empfänger, was der Aufruf tatsächlich tut
([ERC-7730](/de/docs/clear-signing)). Ein Aufruf, den wir nicht dekodieren können,
wird **als nicht dekodierbar markiert** und nicht stillschweigend so dargestellt, als
wäre er in Ordnung. Die Bybit-Payload war ein `delegatecall`, der eine
Implementierungsadresse austauschte; genau so etwas sollte einen Unterzeichner
schlagartig innehalten lassen – und dass es hinter einer freundlichen Zusammenfassung
verborgen war, ist der Grund, warum es das nicht tat.

Zwei Grenzen, die man genau benennen muss. Eine dApp kann von Vela nicht direkt einen
`delegatecall` verlangen – die Anfragen, die eine Seite stellen kann, erzeugen
gewöhnliche Aufrufe –, die Bybit-Payload selbst könnte also nicht auf diesem Weg
ankommen. Aber eine Seite *kann* einen Aufruf von deinem Safe an sich selbst
anfordern: `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`.
Jeder davon übergibt, einmal signiert, das Konto so vollständig wie die
Bybit-Payload – ein aktiviertes Modul kann danach selbst einen `delegatecall`
ausführen. Vela dekodiert solche Aufrufe, blockiert sie aber noch nicht; **lehne jede
Anfrage ab, deren Ziel deine eigene Wallet-Adresse ist.** Und würde Velas eigener Code
ersetzt, wie der von `Safe{Wallet}`, dann wäre auch die Dekodierung die des
Angreifers – dafür ist der nächste Punkt da.

**Ein unabhängiger Weg, der die Oberfläche prüfen kann.** Vela hat eine
[Signaturseite](/de/docs/clear-signing-self-host) ohne Build-Schritt und ohne
Abhängigkeiten gebaut, die die Anfrage selbst dekodiert und die WebAuthn-Signatur
selbst durchführt – ein einzelner Ordner statischer Dateien, den du von vorn bis
hinten lesen, selbst ausliefern oder als Browser-Erweiterung laden kannst. Ihr Zweck
ist, eine zweite Meinung zu sein, die nicht die Lieferkette der Haupt-App teilt.
*Stand: gebaut und getestet; nicht veröffentlicht, und noch keine Vela-App schickt
Anfragen an sie.* Diese Seite sagt es klar, sobald sich das ändert.

**Keine Admin-Rolle, die man uns abnehmen könnte.** Velas Konten sind
[unveränderte Safe v1.4.1](/de/docs/account-contract), und Vela hat darauf keine
privilegierte Rolle – keinen Admin-Schlüssel und keinen eigenen Upgrade-Pfad, zu dessen
Nutzung man uns zwingen oder den man kompromittieren könnte. Sei dir im Klaren
darüber, was das *nicht* beseitigt: Der Baustein, den die Bybit-Angreifer nutzten – ein
von einem Eigentümer signierter `delegatecall`, der die Implementierung des Kontos
umschreibt –, existiert weiterhin in jedem Safe, auch in denen von Vela (Velas eigene
gebündelte Transaktionen nutzen `delegatecall` in das MultiSend von Safe). Es braucht
eine gültige Signatur von einem deiner Schlüssel. Was dich davor schützt, dir eine
abschwatzen zu lassen, sind die Dekodierung oben und die unabhängige Prüfung.

**Eine frische Bestätigung für jede Signatur.** Jede Signatur braucht die eigene
Bestätigung deines Schlüssels – Face ID, Fingerabdruck, PIN oder Berührung und PIN am
Sicherheitsschlüssel. Es gibt keinen langlebigen Sitzungsschlüssel, also auch kein
Zeitfenster, in dem etwas ohne dich in deinem Namen signieren kann.

**Selbst hosten als letzte Absicherung.** Die Apps und die Backend-Dienste sind Open
Source. Willst du unserer Build-Pipeline gar nicht vertrauen, bau die Erweiterung oder
eine App selbst und betreibe die Dienste, die du brauchst – die
[Anleitung zum Selbsthosten](/de/docs/self-hosting) führt dich durch. Damit fällt
unsere Build-Pipeline aus der Vertrauenskette; dem Code, den du baust, vertraust du
weiterhin – also lies ihn.

## Was Vela nicht behauptet

Velas Frontend könnte auf dieselbe Weise kompromittiert werden wie das von
`Safe{Wallet}`. Unser Code ist nicht auditiert. Etwas anderes zu behaupten wäre genau
die Art von Zusicherung, mit der dieser Vorfall hätte Schluss machen sollen.

Was das Design versucht, ist, den Weg zu verengen: die Payload lesbar statt
undurchsichtig machen, keine Admin-Rolle halten, die gegen dich missbraucht werden
könnte, und dir eine Möglichkeit geben, mit etwas zu prüfen, das nicht wir sind. Die
ehrliche Zusammenfassung: **Diese Angriffsklasse wird durch das Design abgeschwächt,
nicht beseitigt**, und die Teile, die den Schutz weiter härten würden, stehen –
unfertig – unter [Audits und bekannte Probleme](/de/docs/security-audits).

## Quellen

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
