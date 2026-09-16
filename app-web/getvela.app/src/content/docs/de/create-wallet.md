---
title: Wallet erstellen
description: Erstelle in etwa einer Minute eine selbstverwahrte Vela-Wallet mit einem Passkey — ohne Seed-Phrase. Deine Wallet ist ein Safe Smart Account mit derselben Adresse in jedem Netzwerk.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Wallet erstellen

Eine Wallet zu erstellen dauert etwa eine Minute und eine biometrische Abfrage.
Öffne die Web-Wallet unter [wallet.getvela.app](https://wallet.getvela.app/) und
wähle **Wallet erstellen**.

## Schritte

1. **Gib deiner Wallet einen Namen.** Wähle einen Namen, an dem du den Account
   später wiedererkennst — auch beim Anmelden auf einem anderen Gerät. Er wird
   neben deinem öffentlichen Schlüssel gespeichert, behandle ihn also als
   öffentlich und schreib nichts Privates hinein.
2. **Bestätige die Grundlagen.** Eine kurze Checkliste hält fest, dass du
   verstehst: Vela ist selbstverwahrt und noch Alpha-Software. Mit Links zur
   [Datenschutzerklärung](/privacy) und den [Nutzungsbedingungen](/terms).
3. **Erstelle deinen Passkey.** Wenn die Abfrage kommt, authentifiziere dich mit
   **Face ID, Touch ID oder Fingerabdruck**. Dabei entsteht ein WebAuthn-Passkey
   (P-256), den dein Gerät hält und Vela nie sieht. Einen Schritt „Seed-Phrase
   notieren“ gibt es nicht, weil es keine Seed-Phrase gibt.
4. **Fertig.** Vela zeigt dir deine Wallet-Adresse, und du bist drin. Du kannst
   sie prüfen und dich anmelden, um in deiner Wallet zu landen.

## Was deine Wallet eigentlich ist

Diesen Teil erklären die meisten Wallets nicht — und er entscheidet darüber, wie
Vela funktioniert.

Deine Vela-Wallet ist ein **Safe Smart Account** (ein Smart Contract), kein
gewöhnliches „extern besessenes Konto“. Dein Passkey ist der Eigentümer dieses
Accounts; eine ERC-4337-Konstruktion lässt dich ihn allein mit Gesicht oder
Fingerabdruck bedienen.

<Callout type="info" title="Deine Adresse ist in jedem Netzwerk dieselbe">
Vela leitet deine Adresse aus dem öffentlichen Schlüssel deines Passkeys ab. Sie
ist auf Ethereum, Base, Arbitrum, Gnosis und jedem anderen unterstützten Netzwerk
identisch. Du gibst überall dieselbe Adresse heraus.
</Callout>

Eine nützliche Folge: Die Adresse ist **kontrafaktisch**. Sie wird berechnet,
bevor on-chain irgendetwas existiert — **du kannst also empfangen, bevor dein
Wallet-Vertrag überhaupt da ist**. Der Vertrag setzt sich bei deiner ersten
Transaktion in einem Netzwerk selbst auf, bezahlt aus seinem eigenen Guthaben.

## Was gerade mit deinen Schlüsseln passiert ist

- Dein Gerät hat ein **Passkey-Schlüsselpaar** erzeugt.
- Der **private Schlüssel** liegt beim Passkey-Dienst deines Betriebssystems
  (iCloud-Schlüsselbund oder Google Passwortmanager), Ende-zu-Ende verschlüsselt
  und über deine Geräte synchronisiert — keine App, auch Vela nicht, sieht ihn je.
- Der **öffentliche Schlüssel und dein gewählter Name** gehen an Velas
  Passkey-Index, der den Schlüssel zusätzlich in einen öffentlich lesbaren
  Eintrag auf der Gnosis Chain schreibt, damit dein Account auf einem neuen Gerät
  wiedergefunden werden kann. Siehe
  [Wiederherstellung und Anmeldung](/de/docs/recovery).

## Nächste Schritte

- [Deine ersten Token empfangen](/de/docs/send-and-receive)
- [Netzwerke und Gebühren verstehen](/de/docs/networks-and-fees)
- [Nachlesen, warum Passkeys das sicher machen](/de/docs/passkeys)
