---
title: Netzwerke und Gebühren
description: "Die 24 in Vela eingebauten Netzwerke, wie du ein weiteres hinzufügst, wie genau die Gebühr einer Transaktion berechnet wird und wer sie erhält, und was passiert, wenn einem Relay das Gas ausgeht."
source: fdc50dbbf13a
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Netzwerke und Gebühren

## Eingebaute Netzwerke

In Vela sind **24 Netzwerke** eingebaut, alle davon Mainnets:

| Netzwerk | Gas bezahlt in | Netzwerk | Gas bezahlt in |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (der native Coin) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (der native Coin) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (kein nativer Coin) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

In den meisten davon kannst du die Gebühr auch in einem USD-Stablecoin zahlen, den
das Relay in diesem Netzwerk akzeptiert (siehe unten).

Deine Wallet hat **in jedem Netzwerk dieselbe Adresse**, weil die Adresse aus deinen
Schlüsseln berechnet wird, nicht aus der Chain.

## Ein weiteres Netzwerk hinzufügen

Unter **Einstellungen → Netzwerke** kannst du jedes EVM-Netzwerk hinzufügen, sofern
es hat, was eine Vela-Wallet braucht: elf Standardverträge (den ERC-4337-EntryPoint
v0.7, die Safe-v1.4.1-Verträge, das 4337- und das Passkey-Modul von Safe, MultiSend,
Multicall3 und zwei deterministische Deployer) und das **RIP-7212**-Precompile, das
Passkey-Signaturen an der Adresse `0x100` prüft. Die Wallet prüft all das –
einschließlich einer echten Signaturprüfung gegen das Precompile –, bevor sie dich das
Netzwerk hinzufügen lässt.

Das Precompile ist eine harte Voraussetzung. Seine Adresse fließt in die Berechnung
jeder Vela-Adresse ein, deshalb gibt es keinen Ersatz-Verifizierer und keine
Möglichkeit, später einen bereitzustellen. Hat eine Chain das Precompile, fehlen ihr
aber einige Verträge, zeigt die [Chain-Einrichtung](/de/chain-setup), was fehlt, und
stellt bereit, was jeder bereitstellen kann. Eine Lücke in der Prüfung: Eine Wallet
mit mehr als einem Schlüssel braucht in dem Netzwerk außerdem die
Passkey-Signer-Factory von Safe, die noch nicht geprüft wird; ohne sie kann dort nur
der erste Schlüssel signieren.

## Wie eine Transaktion bezahlt wird

Vela ist eine ERC-4337-Wallet: Du sendest eine Transaktion nicht selbst ins Netzwerk.
Die App baut eine **UserOperation**, du signierst sie mit einem deiner Schlüssel, und
ein **Relay** reicht sie on-chain ein und streckt das Gas vor. (ERC-4337 nennt diese
Rolle Bundler.) Das Relay wird **innerhalb deiner Operation** bezahlt: Die Zahlung ist
eine Überweisung von deiner Wallet an das Relay, die im selben Batch steckt wie deine
Transaktion und deshalb von deiner Signatur abgedeckt ist. Es gibt keinen Paymaster,
niemand sponsert dein Gas, und niemand kann deine Transaktion wegen einer
Sponsoring-Richtlinie ablehnen.

### Wie hoch die Gebühr ist

<span id="fee"></span>

Der Bestätigungsbildschirm zeigt einen einzigen Betrag, im Gebühren-Coin und in deiner
Anzeigewährung. Er wird so berechnet:

- **Gas, das die Wallet reserviert.** Die Wallet simuliert die Transaktion und
  reserviert mehr Gas, als sie voraussichtlich verbraucht: Die Schätzungen für
  Verifizierung und Ausführung werden jeweils um die Hälfte erhöht, mit Mindestwerten
  (zum Beispiel mindestens 300.000 Gas für die Verifizierung, sobald die Wallet
  bereitgestellt ist, und 2.000.000 für die Transaktion, die sie bereitstellt).
- **Gaspreis.** Der höhere von zwei Werten: der Gaspreis des Netzwerks, wie ihn die
  Wallet selbst abliest, und der Preis des Relays für die gewählte Geschwindigkeit.
  Voreingestellt ist *Schnell*; dafür setzt das Relay etwa das 1,8-Fache der
  Basisgebühr plus die doppelte Prioritätsgebühr an.
- **Gebühr = 3 × reserviertes Gas × Gaspreis**, mindestens etwa 0,01 US-Dollar. Auf
  Tempo ist der Faktor 2, und die Gebühr wird in pathUSD bezahlt.

Die Reserve liegt deutlich über dem, was die Transaktion verbrauchen wird, und der
Preis enthält Spielraum; die Gebühr ist deshalb höher als die On-Chain-Kosten der
Transaktion – und noch höher bei deiner ersten Transaktion in einem Netzwerk, die
zugleich deine Wallet bereitstellt. Das Relay zahlt die echten Kosten und behält den
Rest; erstattet wird nichts. In günstigen Netzwerken sind das Cent-Beträge; im
Ethereum-Mainnet kann es ein spürbarer Betrag sein. Raten musst du nie: Der genaue
Betrag steht vor dem Signieren auf dem Bestätigungsbildschirm.

**Wer sie bekommt.** Die Gebühr geht an den Betreiber des Relays, auf das die Wallet
eingestellt ist – das von Vela, sofern du es nicht änderst. Jede vela-relay-Instanz
funktioniert, auch [eine, die du selbst betreibst](/de/docs/self-hosting#relay), und
die Wallet rechnet mit derselben Formel, egal welches Relay du wählst.

<Callout type="info" title="Was du siehst, zahlst du">
Der Gebührenbetrag und die Adresse, an die er geht, sind Teil der Operation, die du
signierst. Ein Relay, das eines davon ändert, würde deine Signatur ungültig machen –
du zahlst also genau den angezeigten Betrag, nicht mehr, selbst wenn das Gas vor der
Aufnahme in einen Block teurer wird. Ein Gaspreis-Angebot des Relays, das mehr als das
Dreifache des Werts beträgt, den die Wallet selbst abliest, wird abgelehnt.
</Callout>

### Womit du zahlen kannst

- Mit dem **nativen Coin** des Netzwerks, immer.
- Mit einem **USD-Stablecoin** aus der Liste des Relays für dieses Netzwerk, wenn das
  Relay den Preis des nativen Coins ermitteln kann. Stablecoins, von denen du nichts
  hast, werden ausgeblendet.
- Auf **Tempo**, das keinen nativen Coin hat, nur mit **pathUSD**.

Den Gebühren-Coin und die Geschwindigkeit (*Langsam*, *Standard* oder *Schnell*)
wählst du auf dem Bestätigungsbildschirm und in den Einstellungen.

### Deine erste Transaktion in einem Netzwerk

Du kannst in jedem Netzwerk empfangen, bevor deine Wallet dort existiert. Wenn du zum
ersten Mal aus einem Netzwerk sendest, stellt diese Transaktion auch deinen
Wallet-Vertrag bereit (und einen kleinen Signer-Vertrag für jeden zusätzlichen
Schlüssel). Das Gas für die Bereitstellung steckt in der Gebühr dieser Transaktion,
deshalb kostet die erste Sendung in jedem Netzwerk mehr als die folgenden.

Wenn du das **Maximum** eines nativen Coins sendest, behält Vela genug für die Gebühr
zurück.

## Wer das Relay betreibt – und wer die Gebühr bekommt

Standardmäßig nutzt jedes Netzwerk **Velas Relay**, und die Gebühr geht an Vela. Unter
**Einstellungen → Erweitert → Dienst-Endpunkte** kannst du die Wallet auf ein anderes
Relay umstellen; eine Adresse bedient alle eingebauten Netzwerke, und ein eigenes
Netzwerk behält die Relay-Adresse, mit der es hinzugefügt wurde. Das Relay muss ein
[vela-relay](https://github.com/mondaylabsltd/vela-relay) sein – das von Vela oder eines,
das du betreibst –, weil die Wallet das Gebührenangebot über eine Vela-spezifische
Methode abfragt, die allgemeine Bundler wie Pimlico oder Alchemy nicht implementieren.
Wer das Relay betreibt, das du nutzt, erhält die Gebühr; die
[Anleitung zum Selbsthosten](/de/docs/self-hosting#relay) erklärt, wie du eines
betreibst.

Das Relay erhält eine Operation, die bereits signiert ist. Empfänger, Betrag, Gebühr
oder sonst etwas kann es nicht ändern. Es kann die Operation verzögern oder ablehnen,
und es entscheidet, wann sie on-chain landet – bei einem Swap könnte es dir deshalb
grundsätzlich innerhalb deiner Slippage zuvorkommen.

### Wenn einem Relay das Gas ausgeht

Ein Relay bezahlt das Gas in jedem Netzwerk aus seiner eigenen **Treasury**. Ist diese
leer, sagt dir der Sendebildschirm das, bevor du signierst:

- In einem Netzwerk, das Velas Relay bedient, muss der Betreiber des Relays (Vela) sie
  auffüllen; du kannst das melden. Wenn du nicht warten kannst, kannst du
  **freiwillig** selbst einen kleinen Betrag des nativen Coins an die Treasury
  schicken. Dieser Beitrag ist **nicht erstattungsfähig** und bezahlt **nicht** deine
  eigene Transaktion.
- In einem eigenen Netzwerk ist es Sache des Betreibers, das Relay zu finanzieren – und
  das bist womöglich du.

Es gibt kein Gas-Konto pro Wallet und keine Aktivierungseinzahlung: Eine frühere
Version von Vela hatte so etwas, und es existiert nicht mehr.

## Wie Vela die Netzwerke liest

Vela liest Guthaben und simuliert Transaktionen über einen **Pool von
RPC-Endpunkten** pro Netzwerk – die eingebauten, öffentliche Ausweichendpunkte und
alle Anbieterschlüssel oder Endpunkte, die du hinzufügst – und wechselt zum nächsten,
wenn ein Endpunkt langsam ist oder ausfällt. Einen eigenen Endpunkt pro Netzwerk legst
du unter **Einstellungen → Netzwerke** fest. (Die Android-App nutzt derzeit einen
Endpunkt pro Netzwerk, ohne Umschalten, und in der iPhone-App lässt er sich noch nicht
ändern.)

Weiter: [So funktionieren Passkeys](/de/docs/passkeys).
