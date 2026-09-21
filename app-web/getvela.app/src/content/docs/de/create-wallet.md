---
title: Wallet erstellen
description: "Eine Vela-Wallet mit einem bis sieben Schlüsseln erstellen – was jeder Schritt tut, warum die Schlüssel beim Erstellen feststehen, was öffentlich wird und was deine Wallet eigentlich ist."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Wallet erstellen

Eine Wallet zu erstellen dauert ein, zwei Minuten. Öffne die Web-Wallet unter
[wallet.getvela.app](https://wallet.getvela.app/) – oder die Erweiterung, die
Desktop- oder die Handy-App – und wähle **Wallet erstellen**.

## Schritte

1. **Gib deiner Wallet einen Namen.** Der Name hilft dir, sie wiederzuerkennen, und
   er wird zusammen mit deinen Schlüsseln in ein öffentliches Register geschrieben –
   betrachte ihn als öffentlich und schreib nichts Privates hinein.
2. **Bestätige, was passiert.** Du hakst ab, dass deine öffentlichen Schlüssel und
   der Wallet-Name on-chain geschrieben werden, dass deine privaten Schlüssel auf
   deinen Geräten oder Sicherheitsschlüsseln bleiben und dass du den
   [Nutzungsbedingungen](/terms) und der [Datenschutzerklärung](/privacy) zustimmst.
3. **Erstelle deinen ersten Schlüssel.** Wähle, wie: **Dieses Gerät** (Face ID,
   Touch ID, Fingerabdruck, Windows Hello), **Handy oder Tablet** (einen QR-Code
   scannen und den Schlüssel dort erstellen – sofern die App das anbietet) oder
   **USB-Sicherheitsschlüssel**. Dein Gerät erstellt den Passkey und signiert dann
   einmal damit, damit die App weiß, dass der Schlüssel wirklich funktioniert, bevor
   es weitergeht.
4. **Füge weitere Schlüssel hinzu, wenn du willst.** Bis zu sieben insgesamt,
   beliebig gemischt. Jeder einzelne wird allein signieren können. Wird dein einziger
   Schlüssel nirgends synchronisiert – ein Sicherheitsschlüssel oder Windows Hello –,
   verlangt die App einen zweiten, denn mit einem einzigen nicht synchronisierten
   Schlüssel ist die Wallet weg, sobald ein Gerät verloren geht.
5. **Erstellen.** Die App berechnet die Adresse deiner Wallet aus dem vollständigen
   Schlüsselsatz und veröffentlicht den Satz im öffentlichen Register auf der Gnosis
   Chain. Sobald dieser Eintrag on-chain ist, öffnet sich deine Wallet.

<Callout type="warning" title="Wähle deine Schlüssel jetzt">
Deine Adresse wird aus den Schlüsseln berechnet, mit denen du abschließt; Schlüssel
lassen sich deshalb später weder hinzufügen noch entfernen noch ersetzen.
[Signaturschlüssel und Sicherheitsschlüssel](/de/docs/signers) erklärt, warum, und
wie du wählst.
</Callout>

## Was deine Wallet ist

Deine Wallet ist ein **Safe Smart Account** – ein Vertrag, kein einfaches Konto mit
einem einzigen privaten Schlüssel. Deine Schlüssel sind seine Eigentümer, und jeder
einzelne davon kann eine Transaktion autorisieren.
[Der Account-Vertrag](/de/docs/account-contract) listet jeden beteiligten Vertrag
auf.

Die Adresse ist **in jedem Netzwerk dieselbe**, und sie ist **kontrafaktisch**: Sie
wird berechnet, bevor irgendetwas bereitgestellt ist, deshalb kannst du sofort in
jedem Netzwerk Geld empfangen. Der Vertrag stellt sich selbst bereit, wenn du zum
ersten Mal aus einem Netzwerk sendest, und die Gebühr dieser ersten Transaktion
enthält die Bereitstellung. Das Erstellen der Wallet selbst kostet dich nichts.

## Was öffentlich ist

<span id="what-is-public"></span>

Beim Erstellen einer Wallet wird ein dauerhafter Eintrag in einen öffentlichen
Registervertrag auf der Gnosis Chain geschrieben – für alle lesbar und weder änderbar
noch löschbar:

- für jeden Schlüssel der **öffentliche Schlüssel** (nie der private) und die
  **Credential-ID**;
- für jeden Schlüssel das **Authentifikator-Modell** (welcher Passwortmanager oder
  Sicherheitsschlüssel ihn erzeugt hat) und Flags dazu, ob du verifiziert wurdest und
  ob der Schlüssel synchronisiert wird;
- der **Wallet-Name** und eine **Bezeichnung für jeden Schlüssel**;
- die **Wallet-Adresse** und wann sie erstellt wurde;
- die **signierten Registrierungsdaten** selbst.

Velas Public-Key-Index reicht den Eintrag ein und zahlt das Gas dafür, sieht ihn also
als Erster. Nichts darin kann dein Guthaben bewegen; mit ihm findet jeder einzelne
deiner Schlüssel die Wallet auf einem neuen Gerät wieder
([Wiederherstellung](/de/docs/recovery)). Die [Datenschutzerklärung](/privacy)
enthält die vollständige Liste, und die [Register-Seite](/registry) zeigt jeden
Eintrag.

## Nächste Schritte

- [Deine ersten Token empfangen](/de/docs/send-and-receive)
- [Netzwerke und Gebühren verstehen](/de/docs/networks-and-fees)
- [Was tun, wenn du ein Gerät verlierst](/de/docs/recovery)
