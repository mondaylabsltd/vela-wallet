---
title: Vela installieren
description: Vela läuft im Browser — keine Installation, kein App Store. Öffne die Web-Wallet, oder sieh zuerst nach, was dein Gerät für Passkeys braucht.
---

# Vela installieren

Vela läuft **in deinem Browser** — es gibt nichts herunterzuladen und keinen App
Store zu durchlaufen. Öffne die Web-Wallet, und in unter einer Minute hast du
eine Wallet erstellt oder wiederhergestellt.

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Web-Wallet öffnen →</a>

Dieselbe Wallet, aus derselben Codebasis gebaut, läuft auch auf iOS und Android.
**Native Mobile-Apps kommen bald** — und wenn sie da sind, ziehen Passkey und
Wallet einfach mit um, weil der Account on-chain lebt und nicht in einer
einzelnen App.

## Was dein Gerät braucht

Vela signiert mit **Passkeys** (WebAuthn), du brauchst also Gerät und Browser,
die das unterstützen — praktisch alles aus den letzten Jahren:

| Plattform | Passkey-Unterstützung | Synchronisiert über |
| -------- | --------------- | --------- |
| iPhone / iPad / Mac | iOS/iPadOS 16+, aktuelles Safari | iCloud-Schlüsselbund |
| Android | Android 9+, aktuelles Chrome | Google Passwortmanager |
| Desktop | Aktuelles Chrome, Edge, Safari, Firefox | Der Passkey-Dienst deiner Plattform |

Damit deine Wallet dir auf ein neues Gerät folgt, lass die Passkey-Synchronisation
deiner Plattform eingeschaltet (iCloud-Schlüsselbund bei Apple, Google
Passwortmanager bei Android/Chrome). Wie das funktioniert, steht in
[Wiederherstellung und Anmeldung](/de/docs/recovery).

## Die einzigen offiziellen URLs

Vela ist Open Source, und genau das ist der Punkt — aber es heißt auch, dass du
dir sicher sein solltest, beim Echten zu sein. Die einzigen offiziellen Adressen:

- **getvela.app** — diese Seite
- **wallet.getvela.app** — die Wallet

Wenn dich irgendetwas woanders hinschickt, um „Vela zu installieren“, halt an und
vergleiche mit diesen beiden. Der Code ist öffentlich unter
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

Weiter: [Wallet erstellen](/de/docs/create-wallet).
