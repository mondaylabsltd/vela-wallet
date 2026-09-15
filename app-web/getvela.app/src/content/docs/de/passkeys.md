---
title: So funktionieren Passkeys
description: Das Sicherheitsmodell hinter Vela — was ein Passkey ist, wo dein Schlüssel liegt und warum es nichts zum Abfischen gibt.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# So funktionieren Passkeys

Velas gesamtes Sicherheitsmodell steht auf einer Idee: Der Schlüssel, der deine
Wallet kontrolliert, ist ein **Passkey**. Dein Gerät erzeugt ihn, dein
Betriebssystem hält ihn — keine App, auch Vela nicht, kann ihn lesen — und
benutzt wird er nur mit deinem Gesicht oder deinem Fingerabdruck.

## Was ein Passkey wirklich ist

Ein Passkey ist ein Schlüsselpaar aus öffentlichem und privatem Schlüssel, das
dein Gerät erzeugt. Der **private Schlüssel** liegt beim Passkey-Dienst deines
Betriebssystems — typischerweise iCloud-Schlüsselbund bei Apple, Google
Passwortmanager bei Android — Ende-zu-Ende verschlüsselt, sodass keine App ihn
lesen oder kopieren kann. Apps bekommen den Schlüssel nicht; sie dürfen nach
deiner Authentifizierung *dein Gerät bitten, etwas zu signieren*.

Das ist dieselbe Technik, die Apple Pay und deine biometrische Entsperrung
schützt.

<Callout type="info" title="Der entscheidende Punkt">
Eine App — Vela eingeschlossen — kann eine Signatur anfordern, sieht aber nie
deinen privaten Schlüssel. Gesicht oder Fingerabdruck autorisieren dein Gerät zu
signieren; der Schlüssel selbst bleibt beim Betriebssystem, Ende-zu-Ende
verschlüsselt.
</Callout>

## Warum es nichts zum Abfischen gibt

Phishing funktioniert, indem man dich dazu bringt, ein Geheimnis herauszugeben.
Bei einer Seed-Phrase sind das zwölf Wörter, die du in eine gefälschte Seite
tippen kannst. Bei einem Passkey gibt es **kein Geheimnis, das man tippen könnte**.
Eine Betrugsseite kann dich nicht bitten, „deinen Passkey einzugeben“, weil ein
Passkey nichts Eingebbares ist — er ist eine Hardware-Operation, die deine
Biometrie freigibt.

Damit fällt der mit Abstand häufigste Weg weg, auf dem Menschen selbstverwahrtes
Geld verlieren.

## Wie sich eine Signatur anfühlt

1. Du bestätigst eine Transaktion in Vela.
2. Dein Gerät fragt nach Face ID / Touch ID.
3. Dein Gerät signiert die Transaktion mit deinem Passkey.
4. Vela schickt die signierte Transaktion ins Netzwerk.

Dieselbe Geste wie beim Entsperren deines Handys — weil es derselbe
Passkey-Mechanismus ist, den dein Gerät überall sonst schon benutzt.

<Callout type="warning" title="Gerätesicherheit bleibt wichtig">
Ein Passkey schützt hervorragend gegen Angriffe aus der Ferne und gegen Phishing.
Er schützt nicht gegen jemanden, der dein entsperrtes Gerät in der Hand hat und
deine biometrische Prüfung besteht. Setz einen Gerätecode und gib dein
entsperrtes Handy niemandem, dem du nicht vertraust.
</Callout>

## Wo der Rest liegt

Der **öffentliche** Schlüssel deines Passkeys wird in einem kleinen On-Chain-Index
veröffentlicht, damit deine Wallet auf einem neuen Gerät wiederhergestellt werden
kann. Das ist das Thema der nächsten Seite:
[Wiederherstellung und Anmeldung](/de/docs/recovery).
