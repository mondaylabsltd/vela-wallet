---
title: Installer Vela
description: Vela tourne dans votre navigateur — aucune installation, aucun store. Ouvrez le portefeuille web, ou voyez d'abord ce que votre appareil doit prendre en charge pour les passkeys.
---

# Installer Vela

Vela tourne **dans votre navigateur** — il n'y a rien à télécharger et aucun store
à traverser. Ouvrez le portefeuille web et vous pouvez créer ou restaurer un
portefeuille en moins d'une minute.

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Ouvrir le portefeuille web →</a>

Le même portefeuille, issu d'une seule base de code, tourne aussi sur iOS et
Android. **Les applications mobiles natives arrivent bientôt** — et quand elles
sortiront, votre passkey et votre portefeuille suivront, parce que le compte vit
on-chain et non dans une application particulière.

## Ce dont votre appareil a besoin

Vela signe avec des **passkeys** (WebAuthn), il vous faut donc un appareil et un
navigateur qui les prennent en charge — c'est-à-dire à peu près tout ce qui date
de ces dernières années :

| Plateforme | Prise en charge des passkeys | Synchronisé par |
| -------- | --------------- | --------- |
| iPhone / iPad / Mac | iOS/iPadOS 16+, Safari récent | Trousseau iCloud |
| Android | Android 9+, Chrome à jour | Gestionnaire de mots de passe Google |
| Ordinateur | Chrome, Edge, Safari, Firefox à jour | Le service de passkeys de votre plateforme |

Pour que votre portefeuille vous suive sur un nouvel appareil, laissez la
synchronisation des passkeys active (trousseau iCloud chez Apple, gestionnaire de
mots de passe Google sur Android/Chrome). Le fonctionnement est décrit dans
[Récupération et connexion](/fr/docs/recovery).

## Les seules URL officielles

Vela est open source, et c'est bien l'idée — mais cela veut dire aussi que vous
devez vous assurer d'être au bon endroit. Les seules adresses officielles sont :

- **getvela.app** — ce site
- **wallet.getvela.app** — le portefeuille

Si quelque chose vous envoie ailleurs pour « installer Vela », arrêtez-vous et
comparez avec ces deux-là. Le code est public sur
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

Ensuite : [créer votre portefeuille](/fr/docs/create-wallet).
