---
title: Vela installieren
description: "Alle Wege, Vela zu nutzen – Web, Browser-Erweiterung, Desktop und Handy –, was jeder kostet, was jeder kann und was dein Gerät braucht."
source: fa80f5cfdb95
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vela installieren

Dieselbe Wallet läuft an mehreren Orten, und alle öffnen dieselbe Adresse mit
denselben Schlüsseln. Wähle nach Bedarf; du kannst auch mehrere nutzen. Die Downloads
findest du unter [Vela holen](/de/get-started).

| | Was es ist | Kosten | Stand |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) in jedem aktuellen Browser | Kostenlos | Verfügbar |
| **Browser-Erweiterung** | Die Wallet in deiner Symbolleiste; verbindet sich mit dApps | Kostenlos | Herunterladen und von Hand laden; noch nicht im Chrome Web Store |
| **Desktop** | Native App für macOS, Windows und Linux | Kostenlos | Download über „Vela holen“ oder GitHub |
| **iPhone, Android** | Native Apps | Einmalkauf in den Stores | Noch nicht in den Stores; du kannst sie aus dem Quellcode bauen |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Web-Wallet öffnen →</a>

## Web

Nichts zu installieren. Öffne [wallet.getvela.app](https://wallet.getvela.app/),
erstelle eine Wallet oder melde dich an – fertig. Deine Kontenliste wird in diesem
Browser gespeichert; auf einem anderen Gerät meldest du dich einfach wieder mit
einem deiner Schlüssel an.

## Browser-Erweiterung

Für Chromium-Browser: Chrome, Edge und Brave (Chrome 116 oder neuer). Sie legt die
Wallet in die Symbolleiste und lässt dApps sich direkt mit ihr verbinden. Solange sie
nicht im Chrome Web Store ist:

1. Lade die Erweiterung unter [Vela holen](/de/get-started) herunter und entpacke
   sie in einen Ordner, den du behältst – der Browser führt sie von dort aus.
2. Öffne `chrome://extensions` und schalte den **Entwicklermodus** ein.
3. Klicke auf **Entpackte Erweiterung laden** und wähle diesen Ordner.

Es ist dieselbe Wallet: Erweiterung und Web-Wallet verwenden dieselben
`getvela.app`-Passkeys, also öffnen dieselben Schlüssel dieselbe Adresse.

## Desktop

Eine native App, keine Webseite in einem Fenster: **Windows** 10 und 11 (x64 und
ARM), **macOS** 11 oder neuer und **Linux** (.deb, .rpm oder Flatpak, x64 und ARM).

- **Windows** meldet, der Computer sei „durch Windows geschützt“ worden, weil das
  Installationsprogramm noch nicht codesigniert ist. Wähle **Weitere Informationen**
  und dann **Trotzdem ausführen**.
- **macOS**-Builds werden in einem eigenen Schritt von Apple signiert und notarisiert
  und können deshalb hinter den anderen Plattformen zurückliegen. Zeigt der
  Mac-Button „In Kürze“, findest du den neuesten notarisierten Mac-Build auf der
  GitHub-Release-Seite.
- **Linux**: Damit du einen USB-Sicherheitsschlüssel nutzen kannst, muss dein System
  der App Zugriff darauf geben – die .deb- und .rpm-Pakete richten die nötige Regel
  für dich ein.

Unter macOS und Windows hat die Desktop-App einen eingebauten Browser für dApps.
Prüfsummen für jedes Paket stehen auf der
[GitHub-Release-Seite](https://github.com/mondaylabsltd/vela-wallet/releases) – und du
kannst mehr prüfen als nur eine Prüfsumme, siehe unten.

## iPhone und Android

Native Apps für iOS 17.4 oder neuer und Android 10 oder neuer. Sie werden als
Einmalkauf im App Store und bei Google Play verkauft; **in den Stores sind sie noch
nicht**. Der Code ist offen, du kannst sie also kostenlos selbst bauen – mit einem
Unterschied: Ein Build, den du selbst signierst, kann die Passkeys deines Handys
nicht für getvela.app-Wallets verwenden; ein anderes Handy per Scan und
USB-Sicherheitsschlüssel funktionieren aber. Siehe
[Apps selbst bauen](/de/docs/self-hosting#web-app).

## Prüfen, was du heruntergeladen hast

Eine Prüfsumme sagt dir, dass zwei Dateien identisch sind. Sie kann dir nicht sagen,
wer die Datei erstellt hat – und die Liste der Prüfsummen steht auf derselben Seite wie
der Download. Deshalb ist jedes Paket, das wir an ein Release hängen, zusätzlich
**attestiert**: Der Workflow-Lauf, der es gebaut hat, signiert eine Aussage, die die
Datei, den Commit und den Lauf benennt, und GitHub bewahrt sie auf. Die Prüfung kostet
einen einzigen Befehl mit der [GitHub CLI](https://cli.github.com) (einmalig mit
`gh auth login` anmelden; die Prüfung ist kostenlos):

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
```

Sie gibt aus, wer die Datei gebaut hat und aus welchem Commit – oder sie schlägt fehl.
Für diese Antwort muss auf deinem Rechner niemand uns vertrauen: Die Signatur stammt
von GitHub, entstand zur Build-Zeit, und wer eine Datei bloß irgendwo erneut hochlädt,
kann sie nicht erzeugen.

Mac-Images sind mit unserer Developer ID signiert und von Apple notarisiert, was macOS
beim Öffnen für dich prüft. Wenn du selbst nachfragen willst:

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

<Callout type="warning" title="Die Windows-Meldung bleibt">
Attestierung ist keine Codesignatur. Das Windows-Installationsprogramm ist nicht
codesigniert, deshalb hält SmartScreen es weiterhin einmal mit „Der Computer wurde
durch Windows geschützt“ an – wähle <strong>Weitere Informationen</strong> und dann
<strong>Trotzdem ausführen</strong>. Die Attestierung zu prüfen ist die Kontrolle, die
dir sagt, dass die Datei wirklich von uns stammt; bei der Meldung geht es um ein
Zertifikat, das wir nicht gekauft haben.
</Callout>

Pakete, die vor dieser Umstellung veröffentlicht wurden, haben nur ihre Prüfsummen.

## Vela mit dApps nutzen

<span id="dapps"></span>

dApps verbinden sich mit Vela wie mit jeder anderen Browser-Wallet (EIP-1193 und
EIP-6963):

- im Desktop-Browser über die **Vela-Browser-Erweiterung**;
- in der **Desktop-App** (macOS, Windows), der **iPhone-App** und der
  **Android-App** über deren eingebauten Browser.

Die Web-Wallet unter wallet.getvela.app verbindet sich nicht mit dApps, und
WalletConnect gibt es nicht. Jede Anfrage einer dApp wird dekodiert und dir vor dem
Signieren angezeigt – siehe [Klartext-Signatur](/de/docs/clear-signing).

## Was dein Gerät braucht

Vela signiert mit **Passkeys**, und die unterstützt fast jedes Gerät der letzten
Jahre:

| Gerät | Unterstützt |
| --- | --- |
| iPhone, iPad, Mac | iOS/iPadOS 16+, macOS mit aktuellem Safari oder Chrome |
| Android | Ein aktuelles Android mit Google-Play-Diensten oder ein USB-Sicherheitsschlüssel |
| Windows | Windows Hello mit Chrome oder Edge oder ein Sicherheitsschlüssel |
| Linux | Ein Sicherheitsschlüssel oder ein Handy in der Nähe (QR-Code scannen) |

Kann dein Gerät selbst keinen Passkey speichern, nimm ein anderes Handy oder einen
Hardware-Sicherheitsschlüssel. [Signaturschlüssel und Sicherheitsschlüssel](/de/docs/signers)
zeigt, welche Schlüsselarten jede App unterstützt.

## Die einzigen offiziellen Adressen

- **getvela.app** – diese Website und die Downloads
- **wallet.getvela.app** – die Web-Wallet
- **github.com/mondaylabsltd** – der Code und die Release-Pakete

<Callout type="warning" title="Vor der Installation prüfen">
Schickt dich irgendetwas woandershin, um „Vela zu installieren“ oder „deine Wallet zu
verifizieren“, dann hör auf. Vela fragt nie nach einer Seed-Phrase – es hat keine.
</Callout>

Weiter: [Wallet erstellen](/de/docs/create-wallet).
