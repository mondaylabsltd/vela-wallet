---
title: Signaturseite selbst hosten
description: Die abhängigkeitsfreie Seite und Chrome-Erweiterung, die eine Transaktion selbst dekodiert und mit deinem Passkey signiert — wie du eine eigene Kopie betreibst und welche Kopie für deine Wallet signieren kann.
---

# Signaturseite selbst hosten

Vela dekodiert jede Transaktion, bevor du sie freigibst, und diese Arbeit ist
echte Arbeit — aber sie wird von derselben App gemacht, die die Transaktion gebaut
hat. Wird die App oder der Weg, auf dem sie zu dir kommt, manipuliert, kann sie
dir das eine zeigen und das andere signieren lassen. Genau das ist
[Bybit](/de/docs/bybit-attack) passiert.

Die Signaturseite existiert, um das in zwei Teile zu trennen: Die Transaktion
kommt von einem Ort, und Prüfung und Signatur passieren an einem anderen, den du
kontrollierst.

## Was sie ist

Ein Ordner — `app-web/clearsigning` im Repository — der zugleich Webseite und
Chrome-Erweiterung ist. Reines HTML, CSS und JavaScript: kein Framework, kein
Bundler, kein Build-Schritt, keine Abhängigkeiten und keine eigenen
Netzwerkanfragen.

Bekommt sie eine Signaturanfrage, glaubt sie der mitgelieferten Zusammenfassung
nicht. Sie dekodiert die rohe Calldata selbst, berechnet ihren eigenen Digest,
zeigt dir, was die Signatur tatsächlich erlaubt, und fragt erst dann nach deinem
Passkey.

Weil es keinen Build-Schritt gibt, sind die Dateien, die du liest, die Dateien,
die laufen. Du kannst den Ordner gegen das Repository diffen und weißt, was du
ausliefert.

## Welche Kopie für deine Wallet signieren kann

Ein Passkey ist an die Domain gebunden, auf der er erstellt wurde. Deine
Vela-Schlüssel sind unter `getvela.app` registriert, und ein Browser bietet sie
nur einer Seite an, deren Relying Party `getvela.app` ist. Diese eine Regel
entscheidet, welche Art, eine eigene Kopie zu betreiben, dir nützt.

**Als Chrome-Erweiterung — die Variante für deine bestehende Wallet.** Die
Relying Party der Erweiterung ist `getvela.app`, egal woher der Ordner stammt.
Deine bestehenden Schlüssel können also darin signieren, während der Code der
Ordner ist, den du geladen und geprüft hast.

1. Öffne `chrome://extensions` und schalte den **Entwicklermodus** ein.
2. **Entpackte Erweiterung laden**, und wähle den Ordner `app-web/clearsigning`.
3. Das Symbol in der Symbolleiste öffnet die Seite in einem Tab.

**Als Seite auf deiner eigenen Domain oder auf localhost.** Über HTTP(S)
ausgeliefert ist die Relying Party der Seite ihr eigener Hostname — sie kann also
mit Schlüsseln signieren, die unter *diesem* Hostnamen registriert sind, nicht mit
solchen unter `getvela.app`. Das ist der richtige Weg, um die ganze Zeremonie
einmal durchzuspielen, den Desktop-Ablauf zu fahren und für eine Wallet zu
signieren, deren Schlüssel auf deiner eigenen Domain entstand. Es ist kein Weg,
für eine bestehende `getvela.app`-Wallet zu signieren.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Alle Pfade in der App sind relativ, ein Unterverzeichnis auf einem bestehenden
Host funktioniert also auch. `index.html` direkt von der Platte zu öffnen
(`file://`) taugt zum Umschauen — ohne Origin gibt es keine Relying Party, und
nichts kann signiert werden.

## Was sie vor dem Signieren tut

- **Sie dekodiert die Transaktion selbst.** Was der Aufruf tut, an wen und über
  wie viel, aus der Calldata — inklusive verschachtelter Aufrufe in einem Batch.
- **Sie signiert nur einen Digest, den sie selbst berechnet hat.** EIP-191-,
  EIP-712-, SafeOp- und SafeMessage-Digests werden in der Seite berechnet und
  gegen `vela-core` gegengeprüft, denselben Code, den die Wallet benutzt. Ein
  Digest, den sie nicht berechnen kann, ist eine Verweigerung, keine Signatur.
- **Sie prüft, dass es die angefragte Transaktion ist.** Der Aufruf, den die Seite
  wollte, muss tatsächlich in der signierten Operation enthalten sein.
- **Sie verweigert eine unbegrenzte Genehmigung.** Keine Warnung — eine
  Verweigerung, mit einem Hinweis, was stattdessen zu tun ist.
- **Sie sagt, wenn sie etwas nicht lesen kann,** statt eine freundliche
  Zusammenfassung zu zeigen, für die sie nicht geradestehen kann.
- **Sie zeigt Adresse und Identicon des Accounts** und zeigt keinen
  Empfängernamen, den derjenige geliefert hat, der die Signatur will. Alles, was
  der Anfragende kontrolliert, wird entweder weggelassen oder als seine Angabe
  gekennzeichnet.

## Was sie bewusst nicht hat

- **Keine Editoren.** Die Anfrage steht fest, sobald sie ankommt: Du signierst sie
  oder nicht. Eine Gebührenauswahl oder ein Limit-Editor würde die Calldata neu
  schreiben — und genau das ist die Krankheit, gegen die diese Seite existiert.
- **Keine Schlüsselerstellung.** Die Signaturseite kann keinen Passkey erzeugen.
  Einen zu erzeugen hieße, einen anderen Account zu erzeugen.
- **Keine Netzwerkanfragen.** Nichts zu holen heißt nichts abzufangen.

## Wie eine Anfrage sie erreicht

| Anfragender | Kanal |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Eine Seite im selben Browser | `postMessage` |
| Eine Seite im selben Browser, an die Erweiterung | Erweiterungs-Port |
| Eine Desktop-App auf demselben Rechner | URL-Fragment + Loopback-Callback |
| Ein Handy oder ein anderer Computer | Bluetooth LE (Protokoll implementiert; Funkteil noch nicht auf echter Hardware getestet) |

Das Wire-Format, die Digests und eine Tabelle, woher jedes Element auf dem
Bildschirm stammt, stehen in `PROTOCOL.md` neben dem Code.

## Wann du sie benutzen solltest

Ab dem Tag, an dem der Account Geld hält, dessen Verlust dir wehtäte — und von da
an für jede Signatur. Nicht nur bei großen Beträgen: Eine kleine Genehmigung kann
genug abgeben, um ein Konto zu leeren. Eine Signaturgewohnheit, die man sich für
besondere Anlässe aufhebt, ist an dem Tag, an dem sie gebraucht wird, nicht da.
