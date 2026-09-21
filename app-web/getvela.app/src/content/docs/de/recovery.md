---
title: Wiederherstellung und Anmeldung
description: "Wie du mit einem beliebigen deiner Schlüssel auf einem neuen Gerät zurück in deine Wallet kommst, wo die Wallet nachgeschlagen wird und wo die ehrlichen Grenzen einer Wiederherstellung ohne Seed-Phrase liegen."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Wiederherstellung und Anmeldung

Ohne Seed-Phrase beruht die Wiederherstellung auf zwei Dingen: **einem Schlüssel, den
du noch hast**, und **einem öffentlichen Eintrag darüber, welche Schlüssel zu deiner
Wallet gehören**.

## Was beim Erstellen einer Wallet festgehalten wird

Die Adresse deiner Wallet wird aus allen Schlüsseln berechnet, mit denen du sie
erstellst. Damit jeder einzelne dieser Schlüssel die Wallet später wiederfinden kann,
schreibt das Erstellen einer Wallet einen Eintrag in einen öffentlichen
**Registervertrag** auf der Gnosis Chain: zu jedem Schlüssel den öffentlichen
Schlüssel, die Adresse der Wallet, ihren Namen und die signierten
Registrierungsdaten. Das Register hat keinen Eigentümer, und seine Einträge lassen
sich weder ändern noch löschen. (Die vollständige Liste dessen, was öffentlich ist,
steht unter [Wallet erstellen](/de/docs/create-wallet#what-is-public).)

Velas Public-Key-Index-Dienst reicht diesen Eintrag ein und zahlt das Gas dafür; der
Eintrag selbst liegt on-chain, und jede App kann ihn direkt lesen.

## Auf einem neuen Gerät anmelden

1. Öffne Vela und wähle die Anmeldung.
2. Nutze **einen beliebigen** deiner Schlüssel: einen Passkey, der auf dieses Gerät
   synchronisiert wurde, ein Handy in der Nähe (QR-Code scannen) oder deinen
   Sicherheitsschlüssel.
3. Vela ermittelt aus der Signatur den öffentlichen Schlüssel dieses Schlüssels und
   schlägt ihn nach – zuerst in Velas Index, dann, wenn der Index nicht antwortet, im
   Registervertrag auf Gnosis und danach auf Ethereum – und baut die Wallet neu auf.
   Bevor es sie dir zeigt, prüft es, dass die gefundenen Schlüssel tatsächlich die
   eingetragene Adresse ergeben.

Kontenlisten werden nicht zwischen Geräten synchronisiert; die Anmeldung baut sie neu
auf.

<Callout type="info" title="Wenn weder Index noch Register antworten">
Eine Wallet mit <strong>einem einzigen Schlüssel</strong> lässt sich auf dem Gerät
ganz ohne Server wiederherstellen: Zwei Signaturen dieses Schlüssels reichen, um
seinen öffentlichen Schlüssel zu rekonstruieren und die Adresse neu zu berechnen. Eine
Wallet mit mehreren Schlüsseln braucht den Registereintrag, weil ein Schlüssel der App
nicht sagen kann, welche die anderen waren.
</Callout>

## Kopien des Eintrags

Das Register auf Gnosis ist das, das die Apps zuerst lesen. In den **Einstellungen**
kannst du den Eintrag deiner Wallet außerdem in denselben Registervertrag auf
**Ethereum** kopieren – das Gas zahlst du selbst –, damit der Eintrag auf einer zweiten
Chain existiert. Jeder kann eine solche Kopie anlegen; sie enthält nichts, womit sich
Geld bewegen ließe.

## Die ehrlichen Grenzen

<Callout type="warning" title="Ein verlorener Schlüssel ist verloren">
Sind alle Schlüssel weg, mit denen du die Wallet erstellt hast – die synchronisierten
Passkeys, die Handys, die Sicherheitsschlüssel –, kann niemand die Wallet
wiederherstellen: nicht Vela, nicht Apple oder Google, niemand. Es gibt keine
Seed-Phrase, kein Zurücksetzen durch den Support und keine Hintertür.
</Callout>

Unwahrscheinlich wird das, wenn du mehr als einen Weg hinein hast:

- **Lass die Passkey-Synchronisierung an**, wenn du den Passkey dieses Geräts nutzt.
  Sie bringt den Schlüssel auf ein neues Handy oder einen neuen Computer.
- **Sichere das Konto dahinter.** Wer dein Apple- oder Google-Konto kontrolliert, kann
  womöglich einen synchronisierten Passkey nutzen; gib ihm ein starkes Passwort und
  eigene Wiederherstellungsoptionen.
- **Erstelle die Wallet mit mehr als einem Schlüssel**, zum Beispiel mit dem Passkey
  deines Handys und einem Hardware-Sicherheitsschlüssel an einem sicheren Ort.
  Schlüssel lassen sich nur beim Erstellen der Wallet hinzufügen
  ([warum](/de/docs/signers)). Denk daran: Jeder einzelne Schlüssel kann allein
  signieren – und lässt sich nicht entfernen. Wird einer kompromittiert, zieh dein
  Guthaben in eine neue Wallet um ([was zu tun ist](/de/docs/signers)).

## Was Vela kann und was nicht

- **Kann:** den Index am Laufen halten, damit deine Wallet auf einem neuen Gerät
  schnell gefunden wird.
- **Kann nicht:** dein Guthaben bewegen, deine Wallet einfrieren, Schlüssel hinzufügen
  oder entfernen oder einen verlorenen Schlüssel wiederherstellen. Vela hält deine
  Schlüssel nie.

Weiter: [Klartext-Signatur](/de/docs/clear-signing).
