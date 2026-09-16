---
title: Senden und empfangen
description: Wie du in Vela Token empfängst und sendest — eine Adresse über alle Netzwerke, lesbar signierte Transaktionen und wie Account Abstraction dein Geld tatsächlich bewegt.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Senden und empfangen

## Empfangen

1. Öffne deine Wallet und tippe auf **Empfangen**.
2. Teile deine Adresse — kopiere sie, oder lass den Absender den QR-Code scannen.
3. Sobald die Überweisung on-chain bestätigt ist, erscheint das Guthaben in
   deiner Wallet.

Zwei Dinge lohnt es zu wissen:

- Deine Adresse ist **in jedem unterstützten Netzwerk dieselbe**, du gibst also
  überall dieselbe heraus — achte nur darauf, dass der Absender das richtige
  Netzwerk benutzt.
- Du kannst **empfangen, bevor deine Wallet aufgesetzt ist**. Vela-Accounts sind
  kontrafaktische Smart Accounts: Geld kann an deiner Adresse ankommen, bevor der
  Vertrag auf einer Chain existiert. Er setzt sich bei deiner ersten Sendung dort
  selbst auf.

## Senden

1. Tippe auf **Senden** und wähle den **Token**.
2. Gib **Betrag** (du kannst zwischen Token und Anzeigewährung umschalten) und
   **Empfänger** ein. Wo Vela kann, löst es bekannte Empfänger zu einem Namen auf
   — ein Vela-Account, ein ENS-Name, ein Basename und so weiter.
3. **Prüfen und bestätigen.** Vela zeigt die Überweisung und fragt dann nach
   deinem Passkey (Face ID / Touch ID / Fingerabdruck).

### Was beim Bestätigen passiert

Vela „sendet“ eine Transaktion nicht einfach. Unter der Haube:

1. Es baut eine ERC-4337-**UserOperation** für deinen Safe-Account.
2. Dein Gerät signiert sie nach der biometrischen Prüfung mit einer
   **WebAuthn-Assertion (P-256)**.
3. Die signierte Operation geht an das **Relay**, das sie beim EntryPoint
   einreicht; dein Safe prüft die P-256-Signatur **on-chain** und führt aus.

<Callout type="info" title="Das Relay kann deine Transaktion nicht verändern">
Das Relay bekommt eine <strong>bereits signierte</strong> UserOperation. Es kann
verzögern oder die Weiterleitung ablehnen, aber weder Empfänger noch Betrag noch
irgendein anderes Feld ändern — jede Änderung macht deine Signatur ungültig. Es
sorgt für Erreichbarkeit, es ist kein Verwahrer, und es ist Open Source, du kannst
also dein eigenes betreiben.
</Callout>

### Klartext-Signatur — keine blinden Freigaben

Vor dem Signieren dekodiert Vela die Transaktion mit **ERC-7730**-Deskriptoren
und zeigt die **Absicht** (Senden, Genehmigen, Tauschen …), die **Beträge und
Adressen** sowie einen Risikohinweis — statt undurchsichtiger Hex-Zeichen. Kann
ein Aufruf nicht vollständig dekodiert werden, zeigt Vela eine ausdrückliche
**Blindsignatur-Warnung**, statt so zu tun, als verstünde es ihn. Eine unbegrenzte
Token-Genehmigung wird nicht nur markiert — Vela schreibt sie auf einen endlichen
Betrag um und weigert sich, eine Genehmigung einzureichen, die weiterhin
unbegrenzt wäre.

## Bevor du auf Senden drückst

- **Prüfe die ersten und letzten Zeichen der Adresse.** Schadsoftware, die
  Adressen austauscht, gibt es wirklich.
- **Bestätige das Netzwerk.** Im falschen Netzwerk zu senden ist der häufigste
  teure Fehler. Siehe [Netzwerke und Gebühren](/de/docs/networks-and-fees).
- **Fang bei neuen Empfängern klein an.** Eine winzige Testüberweisung ist eine
  billige Versicherung.

Transaktionen sind unumkehrbar. Es gibt keine Hotline, die eine Sendung an die
falsche Adresse zurückholt — das ist das Wesen der Selbstverwahrung.

## Deinen Verlauf lesen

Guthaben und Verlauf werden live aus einem Pool öffentlicher RPC-Endpunkte
gelesen, mit automatischem Failover. Ist das Netzwerk langsam, dauert der Verlauf
einen Moment — ein Ladekreis heißt „wird noch geholt“, nicht „Geld weg“.
