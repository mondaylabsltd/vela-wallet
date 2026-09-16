---
title: Warum wir Vela gebaut haben
description: Die lange Fassung — wo man zwölf Wörter aufbewahren soll, was Passkeys verändert haben, was wir in den Wallets, die wir benutzten, nicht akzeptieren konnten, und welchen Preis wir stattdessen gewählt haben.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Warum wir Vela gebaut haben

Wir wollten keine weitere Wallet bauen. Angefangen hat es mit einer Frage, die wir
nie sauber beantworten konnten:

> Wo soll man zwölf Wörter eigentlich aufbewahren?

## Die ehrliche Antwort ist ein Screenshot

Leg sie in die Notizen-App, und du bist ein gestohlenes Handy vom Ärger entfernt.
Schreib sie auf Papier, und plötzlich denkst du über Feuer, Wasser, Umzüge,
Mitbewohner, Müllsäcke nach — und darüber, ob das zukünftige Du noch weiß, wo
„der sichere Ort“ war.

Die ehrliche Antwort ist für viele ein Screenshot in der Kamerarolle. Alle wissen,
dass das falsch ist. Sie tun es trotzdem — weil die „richtige“ Antwort im Alltag
zu schwer zu tragen ist.

Eine Seed-Phrase ist ein Geheimnis, das Jahrzehnte gewöhnliches Leben überstehen
muss, ohne je kopiert, fotografiert, ins falsche Feld getippt oder jemandem
Hilfsbereitem am Telefon vorgelesen zu werden. Für einen sehr sorgfältigen
Menschen ist das kein schweres Problem. Für einen Menschen schon.

## Dann veränderten Passkeys, wie sich eine Wallet anfühlen kann

Wir benutzten [Base Account](https://account.base.app) täglich, und mit Face ID zu
signieren fühlte sich selbstverständlich an, wie es Seed-Phrasen nie waren —
weniger wie der Umgang mit Gefahrgut, mehr wie der Rest des Internets.

Aber je länger wir es benutzten, desto mehr stießen wir an Kanten, die sich nicht
ignorieren ließen:

- ein **im Browser erzeugter Wiederherstellungsschlüssel**, dem man einfach
  vertrauen musste,
- **keine eigenen Netzwerke**,
- **keine Möglichkeit, es selbst zu hosten**,
- und das leise Problem, das das größte war: **Verschwindet der Dienst,
  verschwindet die Wallet mit ihm.**

Also bauten wir die Version, von der wir abhängen wollten.

## Was Vela tatsächlich ist

Vela ist **eine Passkey-Wallet, die dir vollständig gehören kann.**

Dein Passkey bleibt dort, wo dein Gerät ihn ohnehin schützt — im
iCloud-Schlüsselbund, im Google Passwortmanager oder auf einem
Hardware-Sicherheitsschlüssel in deiner Hand. Signierst du eine Transaktion,
schickt Vela deinem Gerät eine Challenge; dein Gerät signiert sie und schickt nur
die Signatur zurück. Den Schlüssel selbst sieht Vela nie.

Die meisten Wallets haben noch immer einen gefährlichen Moment, und sei er kurz:
Wörter auf einem Bildschirm, eine Seed-Phrase im Speicher, ein
Wiederherstellungsschlüssel in einem Browser-Tab. Vela ist so gebaut, dass dieser
Moment gar nicht erst entsteht.

<Callout type="info" title="Kein Versprechen — eine Architektur">
Wir kommen an deine Schlüssel nicht heran. Nicht „wir versprechen, es nicht zu
tun“ — es gibt in Vela keinen Codepfad, der es könnte. Die Wallet ist ein
<a href="/de/docs/security-audits">Safe Smart Account</a>, bedient von einer
Signatur, die dein Gerät erzeugt und die wir nur entgegennehmen.
</Callout>

Wir haben Vela **Open Source** gemacht, damit du das selbst prüfen kannst, und
**selbst hostbar**, damit deine Wallet nie davon abhängt, dass unsere Firma online
bleibt. Und wir haben auf unveränderten
[Safe-Verträgen](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
gebaut, weil der langweilige, im Feld erprobte Weg der richtige ist, wenn es um
das Geld anderer Leute geht — dieselben Verträge, die on-chain bereits Milliarden
sichern.

## Der Preis, den wir gewählt haben

Es bleibt ein Preis, und ihn zu verschweigen wäre unehrlich.

Mit Vela zählt dein Apple- oder Google-Konto, denn dort lebt ein synchronisierter
Passkey. Verlierst du dieses Konto oder löschst du den Passkey, gibt es keine
Seed-Phrase, kein Zurücksetzen durch den Support, keine Hintertür.

Aber jede selbstverwahrte Wallet lässt dich wählen, mit welchem Risiko du lieber
lebst. Eine Seed-Phrase kann kopiert, abfotografiert, gephisht oder um ein Uhr
nachts in die falsche Seite getippt werden. Ein Passkey ist anders: Es gibt keine
Wörter zu verraten, kein Geheimnis zum Einfügen und keine gefälschte Seite, die
dich dazu bringen kann, ihn herzugeben. Dein Gerät signiert für die echte Domain,
oder es signiert nicht.

Und die Wahl ist nicht binär. Eine Wallet kann mit **bis zu sieben
Signaturschlüsseln** erstellt werden, von denen jeder einzelne allein signieren
kann — Passkeys auf verschiedenen Geräten, ein Handy in der Nähe, das du scannst,
oder ein USB-/NFC-Sicherheitsschlüssel. Soll deine Wallet gar nicht von einem
Plattformkonto abhängen, kannst du den allerersten Schlüssel zu einem
Hardware-Sicherheitsschlüssel machen. Die einzige Bedingung ist der Zeitpunkt:
Deine Adresse wird aus der gesamten Schlüsselmenge abgeleitet, also werden sie
beim Erstellen der Wallet festgelegt.

<Callout type="warning" title="Was dir das nicht kauft">
Zusätzliche Schlüssel sind ein Weg zurück hinein, kein zweites Schloss. Weil jeder
einzelne Schlüssel signieren kann, schützt ein Hardware-Schlüssel dich davor,
den <em>Zugang zu verlieren</em> — er hält niemanden auf, der bereits einen deiner
Schlüssel übernommen hat. Das ist die ehrliche Form von 1-of-n.
</Callout>

## Deshalb gibt es Vela

Eine Wallet ohne Seed-Phrase zum Verstecken, ohne Wiederherstellungsschlüssel zum
Vertrauen und ohne Firma, von der du hoffen musst, dass es sie für immer gibt.

Wenn du die Behauptungen prüfen statt glauben willst: Das
[Whitepaper](/de/docs/whitepaper) hat die Architektur,
[Audits und bekannte Probleme](/de/docs/security-audits) hat jeden Vertrag, von
dem wir abhängen, und was auditiert ist und was nicht, und der gesamte Code liegt
[auf GitHub](https://github.com/mondaylabsltd/vela-wallet).

Weiter: [Vela installieren](/de/docs/install).
