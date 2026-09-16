---
title: Netzwerke und Gebühren
description: Die 12 Netzwerke, die Vela unterstützt, wie Gasgebühren bei Account Abstraction funktionieren, wer das Relay betreibt und die Gebühren einnimmt, wann du die Aktivierung des Gas-Kontos selbst zahlst und wie Vela RPC-Endpunkte auswählt.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Netzwerke und Gebühren

## Unterstützte Netzwerke

Vela bringt **12 EVM-Netzwerke** mit:

| Netzwerk | Natives Gebühren-Token |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Deine Wallet hat **auf allen dieselbe Adresse**, du gibst also überall dieselbe
heraus.

Du kannst auch **eigene Netzwerke hinzufügen** (Einstellungen → Netzwerke). Weil
Vela eine Smart-Account-Wallet ist, muss ein Netzwerk die Verträge bereitstellen,
auf die Vela baut — den ERC-4337 EntryPoint, die Safe-Verträge und die
**P-256-Precompile (RIP-7212)**, die deinen Passkey on-chain prüft. Vela
kontrolliert das automatisch, bevor es dich ein Netzwerk hinzufügen lässt.

<Callout type="info" title="Warum Gnosis so oft auftaucht">
Gnosis Chain ist nicht nur eines der 12 Netzwerke, sondern beherbergt auch Velas
**Passkey-Index** — den Vertrag, der deinen öffentlichen Schlüssel und deinen
Account-Namen für die geräteübergreifende Wiederherstellung speichert. Das ist
unabhängig davon, auf welchem Netzwerk du Transaktionen machst.
</Callout>

## Wie Gebühren funktionieren (Account Abstraction)

Vela nutzt **ERC-4337 Account Abstraction**: Eine Transaktion wird nicht direkt
von dir gesendet, sondern als **UserOperation** einem **Relay** übergeben, das sie
on-chain einreicht und für das Gas entschädigt wird. (Die ERC-4337-Spezifikation
nennt diese Rolle *Bundler*. Velas heißt Relay, weil es mehr tut als bündeln: Es
stellt Gebühren in-band und betreibt das Gas-Konto-Protokoll weiter unten — beides
nicht Teil des Standards.) Daraus folgt einiges:

- **Gas wird aus dem Guthaben deiner eigenen Wallet bezahlt** — standardmäßig im
  nativen Token des Netzwerks (ETH, BNB, xDAI …) oder in einem unterstützten
  Stablecoin, wo das Relay einen anbietet; das Gebühren-Asset wählst du auf dem
  Bestätigungsbildschirm. Tempo hat keine native Coin, dort wird Gas immer in
  USD-Stablecoins abgerechnet. Es gibt keinen ERC-4337-**Paymaster**, der jede
  Transaktion sponsert — oder blockiert. (Die einmalige *Gas-Konto-Aktivierung*
  kann Vela für neue Nutzer übernehmen; das ist etwas anderes und steht unten.)
- **Das Relay nennt den Gaspreis** — es ist die einzige Quelle der Wahrheit, und
  die Wallet zeigt genau dieses Angebot und signiert genau das, was sie zeigt.
  Es gibt keine Geschwindigkeitsauswahl: Jede Transaktion wird mit hoher Priorität
  eingereicht.
- Die Gesamtsumme sind die **Netzwerkkosten plus die Servicegebühr des Relays**,
  mit einer kleinen Mindestgebühr bei sehr günstigen Transaktionen. Das Angebot
  des Relays ist der Preis — es gibt keine separate Gebührentabelle. Ein Teil geht
  an die Validatoren der Chain, der Rest an das Relay, das das Gas vorstreckt und
  die Infrastruktur betreibt.
- Der Bestätigungsbildschirm zeigt die **geschätzte Gebühr** im Gebühren-Asset und
  in deiner Anzeigewährung, bevor du signierst. Der genannte Betrag und sein
  Empfänger sind Teil dessen, was du signierst — das Relay bekommt also exakt das,
  was angezeigt wurde; eine geänderte Zahl würde deine Signatur ungültig machen.

## Wer das Relay betreibt — und wer die Gebühren bekommt

Jedes Netzwerk zeigt auf ein Relay. Standardmäßig ist das **Velas eigenes Relay**,
und du kannst den Endpunkt unter _Einstellungen → Erweitert → Service-Endpunkte_
ersetzen. Ein Endpunkt gilt für alle eingebauten Netzwerke; ein eigenes Netzwerk
behält die Relay-URL, die du beim Hinzufügen angegeben hast.

Eine ehrliche Einschränkung zur Kompatibilität: Die App holt Gebühren über eine
Vela-spezifische RPC-Methode (`vela_getInBandGasQuote`), und ohne sie scheitert
der Sendevorgang. Der Endpunkt, auf den du zeigst, muss also
[vela-relay](https://github.com/mondaylabsltd/vela-relay) betreiben — Velas
Instanz oder eine, die du selbst hostest. Ein generischer ERC-4337-Bundler wie
**Pimlico** oder **Alchemy** implementiert diese Methode nicht und funktioniert im
aktuellen Release deshalb nicht durchgängig.

Wer für ein Netzwerk das Relay betreibt, **kassiert die Gebühren dieses
Netzwerks** — den Relay-Aufschlag auf jede Transaktion und die Einzahlung zur
Gas-Konto-Aktivierung. Betreibe dein eigenes vela-relay, und diese Gebühren
finanzieren deine Infrastruktur statt Velas; an Traffic, den du woanders hin
leitest, verdient Vela nichts.

<Callout type="warning" title="Das Gas-Konto gehört zum vela-relay-Protokoll">
Der Schritt **Gas-Konto-Aktivierung** stattet auf jedem Netzwerk ein eigenes
Relay-Konto für deine Wallet aus. Zeigt dein Endpunkt auf ein selbst gehostetes
vela-relay, füllt die Einzahlung das Konto deines eigenen Relays, nicht das von
Vela.
</Callout>

### Das Gas-Konto aktivieren (Vela Relay)

Auf Velas Relay **aktiviert deine erste Transaktion in jedem Netzwerk ein eigenes
Gas-Konto**. Die App bittet zuerst die Kasse des Relays, das für dich zu
übernehmen — das passiert still im Sendevorgang, und eine gesponserte Wallet
bekommt nie einen Finanzierungsbildschirm zu sehen. Erst wenn das abgelehnt wird,
zeigt die App eine Aufforderung zum Auffüllen: Du schickst einen kleinen Betrag
des nativen Tokens an die angezeigte Gas-Konto-Adresse, und sie sagt dir, warum es
diesmal kein Sponsoring gab.

**Die Aktivierungsgebühr zahlst du selbst**, wann immer kein kostenloses Sponsoring
angeboten wird — nämlich wenn:

- **Velas Kasse für dieses Netzwerk leer oder knapp ist** — der freie Topf ist auf
  dieser Chain vorübergehend aufgebraucht.
- **Du dein Freikontingent ausgeschöpft hast** — Sponsoring ist pro Wallet
  gedeckelt, jenseits der ersten Male zahlst du selbst.
- **Velas Relay dieses Netzwerk gar nicht finanziert** — z. B. **eigene oder
  Testnetzwerke, die du selbst hinzugefügt hast** und für die Vela keine Kasse
  führt. (Leite sie an dein eigenes Relay, wenn du die Aktivierung lieber ganz
  überspringen willst.)

Die Aktivierungseinzahlung ist **nicht erstattungsfähig** — sie ist das
Startguthaben des Relay-Kontos und füllt sich über die Zeit aus Gas-Erstattungen
auf, kann sich aber trotzdem leeren und später eine **erneute Aktivierung**
brauchen. Auch bei einem Service-Upgrade kann sich die Relay-Adresse ändern, was
eine frische Aktivierung nötig macht.

Die Gebühr geht von deinem Guthaben im gewählten **Gebühren-Asset** ab —
standardmäßig dem nativen Token. Wird eine Sendung wegen Gas blockiert, reicht
dein Guthaben in diesem Gebühren-Asset nicht für die Gebühr; wo das Relay
Stablecoin-Gas anbietet, kann ein Wechsel des Gebühren-Assets auf dem
Bestätigungsbildschirm das lösen.

Sendest du den **Maximalbetrag** eines nativen Tokens, legt Vela automatisch genug
für Gas zurück, damit die Transaktion nicht scheitert.

## Wie Vela mit jedem Netzwerk spricht

Vela liest Guthaben und reicht Transaktionen über einen **Pool von
RPC-Endpunkten** ein, nicht über einen einzelnen Anbieter. Es sammelt Endpunkte
aus mehreren Quellen, bewertet sie nach Latenz und Zuverlässigkeit und **wechselt
automatisch**, wenn einer langsam oder ausgefallen ist — schlechte Endpunkte
werden vorübergehend auf die Bank gesetzt, damit ein einzelner wackliger Node nie
die ganze App lahmlegt.

Weiter: [So funktionieren Passkeys](/de/docs/passkeys).
