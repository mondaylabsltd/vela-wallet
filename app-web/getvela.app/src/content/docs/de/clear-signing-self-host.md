---
title: Trusted Signer
description: "Eine Seite aus einer einzigen Datei unter sign.getvela.app, die eine Anfrage selbst dekodiert und mit deinem Passkey signiert – was sie prüft, welche Apps sie nutzen und wie du sie nachbaust oder eine eigene Kopie betreibst."
source: 3a605b8d68ee
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela dekodiert jede Transaktion, bevor du sie freigibst, und diese Dekodierung ist
ehrliche Arbeit – aber es ist Arbeit derselben App, die die Transaktion gebaut hat.
Wird die App oder der Weg, auf dem sie zu dir kommt, manipuliert, kann sie dir das eine
zeigen und das andere signieren. Genau das ist [Bybit](/de/docs/bybit-attack)
passiert.

Den Trusted Signer gibt es, um das in zwei Teile zu trennen: Die App übergibt nur die
Anfrage, und die Prüfung und die Signatur passieren auf einer eigenen Seite – einer,
die du von vorn bis hinten lesen, Byte für Byte nachbauen oder selbst betreiben kannst.

## Wo er läuft

Die offizielle Seite wird von **sign.getvela.app** ausgeliefert. Die Desktop-App (macOS,
Windows, Linux), die iPhone- und die Android-App können ihr eine Anfrage schicken: Die App
öffnet die Seite in einem Browser-Tab, mit der Anfrage im Link, du prüfst sie dort und
signierst mit deinem Passkey, und die Seite gibt die Signatur über einen
`velawallet://`-Link an die App zurück. Die Web-Wallet kann ihn nicht nutzen.

Er ist nur aktiv, wenn du ihn wählst: Du legst ihn als deine Art zu signieren fest, wenn
du eine Wallet erstellst oder dich anmeldest, und von da an läuft jede Signatur für diese
Wallet auf diesem Gerät über ihn. Er kann auch die Schlüssel der Wallet erstellen. Auf
`sign.getvela.app` nutzt er dieselben `getvela.app`-Passkeys wie die Apps.

<Callout type="info" title="Bisher getestet">
Aufgezeichnete Durchläufe von Anfang bis Ende gegen die veröffentlichte Seite: Android und
Windows 11 (eine Wallet erstellen und sich anmelden). Die Apps für macOS, Linux und iPhone
sind auf dieselbe Weise angebunden; für keine davon gibt es bisher einen aufgezeichneten
vollständigen Durchlauf.
</Callout>

## Was er tut, bevor er signiert

- **Er dekodiert die Anfrage selbst.** Was der Aufruf tut, an wen und über welchen
  Betrag, aus der Calldata – einschließlich Aufrufen, die in einem Batch verschachtelt
  sind.
- **Er signiert nur einen Digest, den er selbst berechnet hat.** EIP-191-, EIP-712-,
  SafeOp- und SafeMessage-Digests werden in der Seite berechnet und nie vom Anfragenden
  übernommen; Tests gleichen die SafeOp- und SafeMessage-Digests mit `vela-core` ab, dem
  Code, den die Wallet nutzt, und die App lehnt eine Signatur über jeden anderen Digest
  ab als den, den sie selbst berechnet hat.
- **Er prüft, dass die Transaktion die angeforderte ist.** Der Aufruf, den die Website
  angefordert hat, muss tatsächlich in der Operation stecken, die signiert wird, sonst
  lehnt die Seite ab.
- **Er sagt, wenn eine Freigabe unbegrenzt ist.** Einen Betrag kann er nicht ändern –
  er signiert die Bytes, die angekommen sind, oder gar nichts –, deshalb wird eine
  unbegrenzte Freigabe oder ein unbegrenztes Permit (auf dieser Seite 2^128 oder mehr)
  rot und mit genau dieser Begründung angezeigt und kann unverändert signiert werden;
  eine On-Chain-Obergrenze wählst du auf dem eigenen Freigabebildschirm der Wallet,
  bevor die Anfrage hier ankommt. Eine Freigabe für eine ganze NFT-Sammlung wird
  abgelehnt.
- **Er lehnt ab, wofür er nicht geradestehen kann:** `eth_sign`, eine Methode, die er
  nicht kennt, einen Token, der an seinen eigenen Vertrag gesendet wird, eine
  Operation, die er nicht lesen kann, und eine Anmeldung, deren Challenge der
  Anfragende mitgeliefert hat.
- **Er zeigt die Adresse des Kontos und ein in der Seite berechnetes Identicon.**
  Empfänger und Verträge werden nie anhand der Anfrage benannt – nur die eigene,
  geprüfte Tabelle der Seite kann einen Vertrag benennen. Der eigene Name des Kontos,
  den die App mitschickt, damit du den richtigen Passkey wählen kannst, steht neben
  seiner Adresse.
- **Er verlangt bei jeder Signatur eine Nutzerverifizierung** (deinen Fingerabdruck,
  dein Gesicht oder deine PIN).

## Was er absichtlich nicht hat

- **Keine Editoren.** Die Anfrage steht fest, wenn sie ankommt: Du signierst sie oder
  nicht. Eine Gebührenauswahl oder ein Editor für Freigabelimits würde die Calldata
  umschreiben – genau das Übel, das diese Seite verhindern soll.
- **Keinen Netzwerkzugriff.** Die Seite ist eine einzige Datei, deren Content Security
  Policy (`default-src 'none'`) in ihren eigenen Bytes steckt; sie kann also nichts
  abrufen, keine Verbindung öffnen und kein Bild laden. Das Einzige, was sie verlässt,
  ist ihre Antwort, wenn sie dem Callback-Link in der Anfrage folgt (`velawallet://`,
  wenn eine Vela-App angefragt hat). Token-Logos werden als Buchstaben gezeichnet.

## Was die App im Gegenzug prüft

Auch die App vertraut der Seite nicht. Sie akzeptiert eine Signatur nur, wenn die
signierte Challenge der Digest ist, **den die App berechnet hat**, die
Nutzerverifizierung stattgefunden hat, der Schlüssel einer der Schlüssel deiner Wallet
ist und die P-256-Signatur mit diesem Schlüssel gültig ist.

## Jede veröffentlichte Version, überprüfbar

Jede Version wird aus `app-web/trusted-signer/src/` reproduzierbar zu einer einzigen
Datei gebaut – Bun und Node erzeugen dieselben Bytes – und unter ihrer eigenen Adresse
veröffentlicht, `sign.getvela.app/b/<sha256>/sign.html`, neben jeder früheren Version.
Die Liste steht unter `sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

Beim Start ruft die Desktop-App die veröffentlichte Version ab, die sie öffnen wird,
berechnet ihren Hash und vergleicht ihn mit den Versionen, die in sie eingebaut sind.
Das Ergebnis wird nur protokolliert, und eine Seite, die nicht übereinstimmt, wird
trotzdem geöffnet. Die Handy-Apps prüfen das noch nicht.

## Eine eigene Kopie betreiben

In den Einstellungen steht die Adresse der Seite, die deine Apps öffnen, sodass du sie
auf deine eigene Bereitstellung verweisen lassen kannst: jede HTTPS-Adresse oder
`localhost` zum Testen. Baue sie mit `bun samples/build-single.mjs` (oder `node`) und
kopiere `dist/` auf deinen Host.

Eine Kopie auf deiner eigenen Domain signiert mit Passkeys, die für **diese** Domain
erstellt wurden, nicht mit den `getvela.app`-Passkeys – sie ist also ein Weg, eine
Wallet zu erstellen und zu nutzen, deren Schlüssel unter deiner Domain liegen, und kein
Weg, für eine bestehende `getvela.app`-Wallet zu signieren. Alle Schlüssel einer Wallet
gehören zu ein und derselben Domain.

Der Code und die Skripte, die ihn bauen und prüfen, liegen in
`app-web/trusted-signer/`.
