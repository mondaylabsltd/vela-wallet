---
title: Signaturseite selbst hosten
description: "Die abhängigkeitsfreie Seite und Chrome-Erweiterung, die eine Transaktion selbst dekodiert und mit deinem Passkey signiert – wie du eine eigene Kopie betreibst und welche Kopie für deine Wallet signieren kann."
source: c81389ef7aa2
---

# Signaturseite selbst hosten

Vela dekodiert jede Transaktion, bevor du sie freigibst, und diese Dekodierung ist
ehrliche Arbeit – aber es ist Arbeit derselben App, die die Transaktion gebaut hat.
Wird die App oder der Weg, auf dem sie zu dir kommt, manipuliert, kann sie dir das eine
zeigen und das andere signieren. Genau das ist [Bybit](/de/docs/bybit-attack)
passiert.

Die Signaturseite gibt es, um das in zwei Teile zu trennen: Die Transaktion kommt von
einem Ort, und die Prüfung und die Signatur passieren an einem Ort, den du
kontrollierst.

**Stand:** gebaut und lokal getestet; **nicht veröffentlicht**, und **noch keine
Vela-App schickt Anfragen an sie**. Heute kannst du sie lesen, starten und mit den
Demo-Anfragern im Ordner `samples/` ausprobieren. Um sie für echte Signaturen zu
nutzen, müssen die Apps ihre Anfragen an sie weiterleiten, und das ist noch zu bauen.

## Was sie ist

Ein Ordner – `app-web/clearsigning` im Repository –, der zugleich eine Webseite und eine
Chrome-Erweiterung ist. Reines HTML, CSS und JavaScript: kein Framework, kein Bundler,
kein Build-Schritt, keine Abhängigkeiten und keine Daten von einem Server – ihre
einzige Anfrage gilt dekorativen Token-Logos.

Bekommt sie eine Signaturanfrage, traut sie der mitgelieferten Zusammenfassung nicht.
Sie dekodiert die rohe Calldata selbst, berechnet ihren eigenen Digest, zeigt dir, was
die Signatur tatsächlich autorisieren wird, und fragt erst dann deinen Passkey.

Weil es keinen Build-Schritt gibt, sind die Dateien, die du liest, die Dateien, die
laufen. Du kannst den Ordner mit dem Repository vergleichen und weißt, was du
auslieferst.

## Welche Kopie für deine Wallet signieren kann

Ein Passkey ist an die Domain gebunden, auf der er erstellt wurde. Deine
Vela-Schlüssel sind unter `getvela.app` registriert, und ein Browser bietet sie nur
einer Seite an, deren Relying Party `getvela.app` ist. Diese eine Regel entscheidet,
welche Art, deine eigene Kopie zu betreiben, dir etwas nützt.

**Als Chrome-Erweiterung – der Weg, sie mit deinen bestehenden Schlüsseln zu
nutzen.** Die Relying Party der Erweiterung ist `getvela.app`, egal woher der Ordner
stammt; deine bestehenden Schlüssel können darin also signieren, während der Code der
Ordner ist, den du geladen und geprüft hast.

1. Hol dir den Ordner: `git clone https://github.com/mondaylabsltd/vela-wallet`
   (darin liegt `app-web/clearsigning`).
2. Öffne `chrome://extensions` und schalte den **Entwicklermodus** ein.
3. Klicke auf **Entpackte Erweiterung laden** und wähle den Ordner
   `app-web/clearsigning`.
4. Das Symbol in der Symbolleiste öffnet die Seite in einem Tab.

**Als Seite auf deiner eigenen Domain oder auf localhost.** Über HTTPS (oder von
localhost) ausgeliefert, ist die Relying Party der Seite ihr eigener Hostname – sie kann
also mit Schlüsseln signieren, die unter _diesem_ Hostnamen registriert sind, nicht mit
Schlüsseln unter `getvela.app`. Das macht sie zum richtigen Weg, die ganze Zeremonie
durchgängig auszuprobieren, den Desktop-Ablauf zu testen und für eine Wallet zu
signieren, deren Schlüssel auf deiner eigenen Domain erstellt wurde. Für eine
bestehende `getvela.app`-Wallet kann sie so nicht signieren.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Alle Pfade in der App sind relativ, deshalb funktioniert auch ein Unterverzeichnis auf
einem bestehenden Host, und `index.html` direkt von der Festplatte zu öffnen
(`file://`) reicht zum Umsehen – ohne Origin gibt es keine Relying Party, und nichts
lässt sich signieren.

## Was sie tut, bevor sie signiert

- **Sie dekodiert die Transaktion selbst.** Was der Aufruf tut, an wen und über
  welchen Betrag, aus der Calldata – einschließlich Aufrufen, die in einem Batch
  verschachtelt sind.
- **Sie signiert nur einen Digest, den sie selbst berechnet hat.** EIP-191-, EIP-712-,
  SafeOp- und SafeMessage-Digests werden in der Seite berechnet und mit `vela-core`
  abgeglichen, demselben Code, den die Wallet nutzt. Ein Digest, den sie nicht
  berechnen kann, führt zur Ablehnung, nicht zu einer Signatur.
- **Sie prüft, dass die Transaktion die angeforderte ist.** Der Aufruf, den die Website
  angefordert hat, muss tatsächlich in der Operation stecken, die signiert wird.
- **Sie lehnt eine Freigabe in „unbegrenzter“ Höhe ab.** Keine Warnung – eine
  Ablehnung, mit einem Hinweis, was du stattdessen tun kannst.
- **Sie sagt, wenn sie etwas nicht lesen kann,** statt eine freundliche
  Zusammenfassung zu zeigen, für die sie nicht geradestehen kann.
- **Sie zeigt die Adresse und das Identicon des Kontos** und zeigt keinen
  Empfängernamen, den der Anfragende mitgeliefert hat. Alles, was der Anfragende
  kontrolliert, wird entweder verworfen oder als seine Angabe gekennzeichnet.

## Was sie absichtlich nicht hat

- **Keine Editoren.** Die Anfrage steht fest, wenn sie ankommt: Du signierst sie oder
  nicht. Eine Gebührenauswahl oder ein Editor für Freigabelimits würde die Calldata
  umschreiben – genau das Übel, das diese Seite verhindern soll.
- **Keine Schlüsselerstellung.** Die Signaturseite kann keinen Passkey erstellen. Einen
  zu erstellen hieße, ein anderes Konto zu erstellen.
- **Keine Daten aus dem Netz.** Nichts, was sie anzeigt oder signiert, wird abgerufen.
  Das Einzige, was sie lädt, sind Token-Logos als Bilder vom Chain-Daten-Server von
  Vela; schlägt das fehl, tritt ein Buchstabe an ihre Stelle.

## Wie eine Anfrage zu ihr gelangt

| Anfragender                                  | Kanal                                                                      |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Eine Seite im selben Browser                 | `postMessage`                                                              |
| Eine Seite im selben Browser, an die Erweiterung | Erweiterungs-Port                                                      |
| Eine Desktop-App auf demselben Rechner       | URL-Fragment + Loopback-Callback (Demo in `samples/`; die Vela-Desktop-App nutzt das noch nicht) |
| Ein Handy oder ein anderer Computer          | Bluetooth LE (Protokoll implementiert; Funkverbindung noch nicht auf echter Hardware getestet) |

Das Übertragungsformat, die Digests und eine Tabelle, woher jedes Element auf dem
Bildschirm stammt, stehen in `PROTOCOL.md` neben dem Code.

## Wo sie hingehört

Sobald die Apps ihre Anfragen an sie übergeben können, ist die vorgesehene Nutzung
einfach: Ab dem Tag, an dem das Konto Geld hält, dessen Verlust dich schmerzen würde,
geht jede Signatur über eine Seite, deren Code du selbst geladen hast. Nicht nur bei
großen Beträgen – eine kleine Freigabe kann genug aus der Hand geben, um ein Konto zu
leeren. Bis dahin ist die Seite eine Möglichkeit, genau nachzulesen und auszuprobieren,
wie diese zweite Meinung funktionieren wird.
