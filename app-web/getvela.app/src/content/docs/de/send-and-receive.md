---
title: Senden und empfangen
description: "Wie du mit Vela empfängst und sendest – eine Adresse in jedem Netzwerk, Senden an eine oder viele Personen, woher Empfängernamen kommen, was du bestätigst und wie das Relay dein Geld bewegt."
source: 9e280dfc853b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Senden und empfangen

## Empfangen

1. Öffne deine Wallet und tippe auf **Empfangen**.
2. Teile deine Adresse – kopiere sie oder zeig den QR-Code. Die Web-Wallet kann
   außerdem eine Zahlungsanfrage mit Betrag erstellen.
3. Sobald die Überweisung on-chain bestätigt ist, erscheint sie in deinem Guthaben.

- Deine Adresse ist **in jedem Netzwerk dieselbe**, du gibst also nur eine Adresse
  weiter – der Absender muss aber trotzdem ein Netzwerk nutzen, das Vela unterstützt
  oder das du hinzugefügt hast.
- Du kannst in einem Netzwerk **empfangen, bevor deine Wallet dort bereitgestellt
  ist**. Sie stellt sich selbst bereit, wenn du dort zum ersten Mal sendest.

## Senden

1. Tippe auf **Senden** und wähle den **Token**.
2. Gib den **Betrag** ein (im Token oder in deiner Anzeigewährung) und die **Adresse
   des Empfängers** – einfügen, QR-Code scannen oder einen Kontakt wählen.
3. **Prüfen.** Vela zeigt, was passieren wird, die Gebühr und – falls vorhanden – den
   Namen, den es für den Empfänger gefunden hat.
4. **Bestätige** mit einem deiner Schlüssel – Face ID, Fingerabdruck, PIN oder
   Berührung und PIN an deinem Sicherheitsschlüssel.

### An viele senden oder zusammenführen

- **Aufteilen** – einen Token in einer einzigen Transaktion an mehrere Personen
  senden. Du kannst eine Liste einfügen oder eine Tabelle importieren und Beträge in
  deiner Währung eingeben.
- **Zusammenführen** – mehrere Token in einer einzigen Transaktion an eine Adresse
  senden.

So oder so signierst du einmal, und für die Transaktion fällt eine einzige Gebühr an.

### Namen für Adressen

Wenn du eine Adresse eingibst, sucht Vela einen Namen dafür: zuerst im eigenen
Register (der Name einer anderen Vela-Wallet), dann in den Reverse-Einträgen von
`.bnb`, `.arb`, `.g`, Basename und ENS, jeweils direkt von der Chain gelesen. Das geht
nur in eine Richtung – es benennt eine Adresse, die du eingegeben hast. Tippst du einen
Namen wie `alice.eth` ein, wird keine Adresse nachgeschlagen. Auch deine gespeicherten
**Kontakte** zeigen ihre Namen.

Ein Name aus diesen Reverse-Einträgen wird nur angezeigt, wenn er **vorwärts auf
dieselbe Adresse auflöst**. Jeder kann seinen eigenen Reverse-Eintrag auf eine
beliebige Zeichenfolge setzen, der Eintrag allein beweist also nichts; die Wallet fragt
den Namensdienst, auf welche Adresse dieser Name zeigt, und zeigt den Namen nur, wenn
beide übereinstimmen. Lässt sich die Prüfung nicht durchführen – ein Endpunkt, der
nicht antwortet, ein Resolver, der fehlschlägt –, siehst du die Adresse und keinen
Namen, nie einen ungeprüften.

### Gebühren-Coin und Geschwindigkeit

Der Bestätigungsbildschirm zeigt die Gebühr im Gebühren-Coin und in deiner Währung.
Du kannst im Coin des Netzwerks zahlen oder, wo das Relay es akzeptiert, in einem
USD-Stablecoin, und eine Geschwindigkeit wählen (voreingestellt ist *Schnell*). Wenn
du das **Maximum** eines nativen Coins sendest, behält Vela genug für die Gebühr
zurück. [Wie die Gebühr berechnet wird](/de/docs/networks-and-fees).

### Was beim Bestätigen passiert

1. Vela baut eine ERC-4337-**UserOperation** für dein Safe, einschließlich der
   Gebührenzahlung an das Relay.
2. Dein Schlüssel signiert sie mit einer **WebAuthn-Assertion (P-256)**, nachdem er
   dich verifiziert hat.
3. Die signierte Operation geht an das **Relay**, das sie beim EntryPoint einreicht;
   dein Safe prüft die P-256-Signatur on-chain und führt sie aus.

<Callout type="info" title="Das Relay kann deine Transaktion nicht ändern">
Das Relay erhält eine Operation, die bereits signiert ist. Empfänger, Betrag und
Gebühr kann es nicht ändern – jede Änderung macht deine Signatur ungültig. Es kann sie
verzögern oder ablehnen, und es entscheidet, wann sie on-chain landet. Es ist Open
Source, und du kannst [ein eigenes betreiben](/de/docs/self-hosting#relay).
</Callout>

Bevor du signierst, dekodiert Vela, was die Transaktion tut, und warnt dich vor dem,
was es nicht dekodieren kann; siehe [Klartext-Signatur](/de/docs/clear-signing).

## Bevor du auf Senden tippst

- **Prüfe Anfang und Ende der Adresse.** Malware, die Adressen austauscht, gibt es
  wirklich – ebenso ähnlich aussehende Adressen, die dir in den Verlauf geschmuggelt
  werden.
- **Prüfe das Netzwerk.** Im falschen Netzwerk zu senden ist ein häufiger, teurer
  Fehler.
- **Fang bei neuen Empfängern klein an.** Eine winzige Testüberweisung ist eine
  günstige Versicherung.

Transaktionen lassen sich nicht rückgängig machen. Niemand kann eine Sendung an die
falsche Adresse zurückholen – das liegt in der Natur der Selbstverwahrung.

## Deine Aktivität

Deine Aktivität verbindet, was du von diesem Gerät gesendet hast, mit
Token-Überweisungen, die aus den Logs der jeweiligen Chain gelesen werden. Eine
einfache Überweisung im nativen Coin an dich, die über einen anderen Vertrag ankommt
(zum Beispiel manche Börsenauszahlungen), erzeugt in manchen Netzwerken kein Log; sie
kann dann in deinem Guthaben auftauchen, ohne in der Aktivität zu erscheinen. Guthaben
werden live über einen Pool von RPC-Endpunkten gelesen, der bei Ausfällen automatisch
umschaltet; ein Ladekreisel bedeutet „wird noch abgerufen“, nicht „Geld weg“.

Weiter: [Netzwerke und Gebühren](/de/docs/networks-and-fees).
