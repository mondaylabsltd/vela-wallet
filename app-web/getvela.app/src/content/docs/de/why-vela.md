---
title: Warum wir Vela gebaut haben
description: "Die lange Fassung – wo man zwölf Wörter aufbewahren soll, was Passkeys verändert haben, was wir an den Wallets, die wir schon benutzten, nicht akzeptieren konnten, und welchen Kompromiss wir stattdessen gewählt haben."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Warum wir Vela gebaut haben

Wir wollten keine weitere Wallet bauen. Angefangen hat es mit einer Frage, die wir
nie sauber beantworten konnten:

> Wo soll man zwölf Wörter eigentlich aufbewahren?

## Die ehrliche Antwort ist ein Screenshot

Leg sie in die Notizen-App, und schon ein gestohlenes Handy kann Ärger bedeuten.
Schreib sie auf Papier, und plötzlich denkst du über Feuer, Wasser, Umzüge,
Mitbewohner und Müllsäcke nach – und darüber, ob dein zukünftiges Ich noch weiß, wo
„der sichere Ort“ war.

Die ehrliche Antwort ist für viele ein Screenshot in der Foto-App. Alle wissen,
dass das falsch ist. Sie tun es trotzdem – weil sich mit der „richtigen“ Antwort im
Alltag kaum leben lässt.

Eine Seed-Phrase ist ein Geheimnis, das Jahrzehnte gewöhnlichen Lebens überstehen
muss, ohne je kopiert, fotografiert, ins falsche Feld getippt oder jemandem
Hilfsbereitem am Telefon vorgelesen zu werden. Für einen sehr sorgfältigen Menschen
ist das kein schweres Problem. Für einen Menschen schon.

## Dann veränderten Passkeys, wie sich eine Wallet anfühlen kann

Wir benutzten [Base Account](https://account.base.app) täglich, und mit Face ID zu
signieren fühlte sich so selbstverständlich an, wie es Seed-Phrasen nie waren –
weniger wie der Umgang mit Gefahrgut, mehr wie der Rest des Internets.

Aber je länger wir es benutzten, desto öfter stießen wir an Grenzen, die sich nicht
ignorieren ließen:

- ein **im Browser erzeugter Wiederherstellungsschlüssel**, dem man einfach vertrauen
  musste,
- **keine eigenen Netzwerke**,
- **keine Möglichkeit, es selbst zu hosten**,
- und das leise Problem, das das größte war: **Verschwindet der Dienst, verschwindet
  die Wallet mit ihm.**

Also bauten wir die Version, auf die wir uns verlassen wollten.

## Was Vela tatsächlich ist

Vela ist **eine Passkey-Wallet, die dir vollständig gehören kann.**

Dein Passkey bleibt dort, wo dein Gerät ihn ohnehin schützt – im
iCloud-Schlüsselbund, im Google Passwortmanager oder auf einem
Hardware-Sicherheitsschlüssel, den du selbst in der Hand hast. Signierst du eine
Transaktion, bittet Vela dein Gerät, sie zu signieren; dein Gerät signiert und
schickt nur die Signatur zurück. Den Schlüssel selbst sieht Vela nie.

Die meisten Wallets haben noch immer einen gefährlichen Moment, und sei er noch so
kurz: Wörter auf einem Bildschirm, eine Seed-Phrase im Speicher, ein
Wiederherstellungsschlüssel in einem Browser-Tab. Vela ist so gebaut, dass dieser
Moment gar nicht erst entsteht.

<Callout type="info" title="Kein Versprechen – eine Architektur">
Wir kommen an deine Schlüssel nicht heran. Nicht „wir versprechen, es nicht zu tun“ –
es gibt in Vela keinen Codepfad, der es könnte; WebAuthn lässt es nicht zu. Die
Wallet ist ein <a href="/de/docs/account-contract">Safe Smart Account</a>, bedient von
einer Signatur, die dein Gerät erzeugt und die wir nur entgegennehmen. Was die App,
mit der du signierst, sehr wohl bestimmt, ist, <em>was</em> dein Schlüssel signieren
soll – deshalb geht das <a href="/de/docs/whitepaper">Bedrohungsmodell</a> so
ausführlich darauf ein.
</Callout>

Wir haben Vela als **Open Source** veröffentlicht, damit du das selbst prüfen kannst,
und **selbst hostbar** gemacht, damit eine bestehende Wallet auch ohne die Server
unserer Firma weiter funktioniert – mit einer Grenze: der Domain, zu der deine Passkeys
gehören. Die [Anleitung zum Selbsthosten](/de/docs/self-hosting) erklärt sie und wie
du sie umgehst. Und wir haben auf unveränderten
[Safe-Verträgen](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
aufgebaut, weil der langweilige, im Feld erprobte Weg der richtige ist, wenn es um das
Geld anderer Leute geht – dieselben Verträge, die on-chain bereits Milliarden sichern.

## Der Kompromiss, den wir gewählt haben

Es bleibt ein Kompromiss, und ihn zu verschweigen wäre unehrlich.

Mit Vela zählt dein Apple- oder Google-Konto, denn dort liegt ein synchronisierter
Passkey. Verlierst du dieses Konto oder löschst du den Passkey, gibt es keine
Seed-Phrase, kein Zurücksetzen durch den Support, keine Hintertür.

Aber jede Wallet zur Selbstverwahrung lässt dich wählen, mit welchem Risiko du lieber
lebst. Eine Seed-Phrase kann kopiert, abfotografiert, gephisht oder um ein Uhr nachts
auf der falschen Seite eingetippt werden. Ein Passkey ist anders: Es gibt keine
Wörter zu verraten, kein Geheimnis zum Einfügen und keine gefälschte Seite, die dich
dazu bringen kann, ihn herauszugeben. Dein Browser bietet ihn nur Seiten auf der
echten Domain an.

Und die Wahl ist nicht binär. Eine Wallet kann mit **bis zu sieben
Signaturschlüsseln** erstellt werden, von denen jeder einzelne allein signieren kann –
Passkeys auf verschiedenen Geräten, ein Handy in der Nähe, das du per Scan verbindest,
oder ein USB-/NFC-Sicherheitsschlüssel. Soll deine Wallet gar nicht von einem
Plattformkonto abhängen, kannst du ausschließlich Hardware-Sicherheitsschlüssel
verwenden – zwei davon, denn eine Wallet kann nicht auf einem einzigen Schlüssel
ruhen, der nirgends synchronisiert wird. Die einzige Bedingung ist der Zeitpunkt:
Deine Adresse wird aus dem gesamten Schlüsselsatz abgeleitet, also werden die
Schlüssel beim Erstellen der Wallet gewählt.

<Callout type="warning" title="Was dir das nicht bringt">
Zusätzliche Schlüssel sind ein Weg zurück hinein, kein zweites Schloss. Weil jeder
einzelne Schlüssel signieren kann, schützt dich ein Hardware-Schlüssel davor, den
<em>Zugang zu verlieren</em> – er hält niemanden auf, der bereits einen deiner
Schlüssel übernommen hat. Das ist die ehrliche Form von 1-of-n.
</Callout>

## Deshalb gibt es Vela

Eine Wallet ohne Seed-Phrase, die du verstecken musst, ohne
Wiederherstellungsschlüssel, dem du vertrauen musst, und ohne Firma, von der du
hoffen musst, dass es sie für immer gibt.

Wenn du die Behauptungen prüfen statt glauben willst: Das
[Whitepaper](/de/docs/whitepaper) beschreibt die Architektur,
[Audits und bekannte Probleme](/de/docs/security-audits) listet jeden Vertrag, von dem
wir abhängen, und was auditiert ist und was nicht, und der gesamte Code liegt
[auf GitHub](https://github.com/mondaylabsltd/vela-wallet).

Weiter: [Vela installieren](/de/docs/install).
