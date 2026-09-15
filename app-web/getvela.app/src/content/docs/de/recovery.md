---
title: Wiederherstellung und Anmeldung
description: Wie du deine Wallet auf einem neuen Gerät ohne Seed-Phrase zurückbekommst — und wo dieses Modell ehrlicherweise an seine Grenzen stößt.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Wiederherstellung und Anmeldung

Der schwierigste Teil einer Wallet ohne Seed-Phrase ist die Wiederherstellung:
Wenn es keine zwölf Wörter gibt, wie kommst du auf einem neuen Handy wieder
hinein? Hier steht genau, wie Vela das löst.

## Wie es funktioniert

Beim Erstellen einer Wallet werden zwei Dinge in Velas **Passkey-Index**
veröffentlicht:

- Der **öffentliche Schlüssel** deines Passkeys (nie der private).
- Der **Name**, den du der Wallet gegeben hast.

Der öffentliche Schlüssel liegt über einen Smart Contract auf der
Gnosis-Blockchain, ist also öffentlich lesbar und hängt nicht daran, dass Velas
Server online bleiben.

Dein **privater** Schlüssel ist derweil ein Passkey, den der Schlüsselbund deiner
Plattform synchronisiert — **iCloud-Schlüsselbund** auf Apple-Geräten, **Google
Passwortmanager** auf Android.

So meldest du dich auf einem neuen Gerät an:

1. Melde dich mit demselben iCloud- oder Google-Konto an, mit aktivierter
   Schlüsselbund-Synchronisation.
2. Öffne Vela und wähle „Anmelden“.
3. Authentifiziere dich mit deinem Passkey. Deine Plattform liefert den
   synchronisierten Passkey, der Index den passenden Account. Deine Wallet ist
   zurück.

Der Index ist ein Cache, kein Single Point of Failure. Ist er einmal nicht
erreichbar und dein Account nicht im lokalen Speicher, kann Vela deinen
öffentlichen Schlüssel auf dem Gerät aus zwei Passkey-Signaturen rekonstruieren
und daraus deine Wallet-Adresse neu ableiten — ganz ohne Server.

<Callout type="info" title="Warum diese Aufteilung">
Der öffentliche Schlüssel im On-Chain-Index lässt jeden — auch eine frische
Installation — deinen Account finden. Der private Schlüssel, synchronisiert vom
Schlüsselbund deiner Plattform, ist das, was Transaktionen tatsächlich
autorisiert. Alles im Index sind öffentliche Daten, und nichts davon kann dein
Geld bewegen — das können nur Signaturen deines Passkeys.
</Callout>

## Die ehrlichen Grenzen

Selbstverwahrung heißt, dass die Verantwortung wirklich bei dir liegt. Das
solltest du wissen.

<Callout type="warning" title="Deine Wiederherstellung hängt am Schlüsselbund deiner Plattform">
Velas geräteübergreifende Anmeldung beruht darauf, dass dein Passkey über
iCloud-Schlüsselbund oder Google Passwortmanager synchronisiert wird. Halte
dieses Konto sicher und seine Wiederherstellungsoptionen aktuell. Verlierst du
<strong>sowohl</strong> deine Geräte <strong>als auch</strong> den Schlüsselbund
deines Plattformkontos, kann Vela deinen privaten Schlüssel nicht neu erzeugen —
by design: wir hatten ihn nie.
</Callout>

Praktischer Rat:

- **Lass die Schlüsselbund-Synchronisation an.** Sie trägt deinen Passkey von
  Gerät zu Gerät.
- **Sichere dein Apple-/Google-Konto** mit einem starken Passwort und eigenen
  Wiederherstellungswegen. Dieses Konto gehört jetzt zur Sicherheit deiner Wallet.
- **Halte nach Möglichkeit mehr als ein Gerät angemeldet**, damit ein verlorenes
  Handy eine Unannehmlichkeit bleibt und keine Krise wird.

## Was Vela kann und was nicht

- **Kann:** dir helfen, deinen Account über den öffentlichen Index wiederzufinden.
- **Kann nicht:** dein Geld bewegen, deine Wallet einfrieren oder einen privaten
  Schlüssel wiederherstellen. Vela hat ihn nie besessen. Genau das ist der Sinn
  von Selbstverwahrung — und der Handel, den du dafür eingehst.
