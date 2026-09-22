---
title: Signataires et clés de sécurité
description: "Un portefeuille Vela peut avoir jusqu'à sept signataires — des passkeys, un téléphone à proximité ou une clé de sécurité de type YubiKey —, et n'importe lequel peut signer. Ils sont choisis à la création du portefeuille ; cette page explique pourquoi, et quoi faire si une clé est compromise."
source: 072891bd4768
---

# Signataires et clés de sécurité

Un portefeuille Vela est un Safe, et un Safe a des propriétaires. Le vôtre peut en
avoir **jusqu'à sept**, avec un seuil de **un** : n'importe quel signataire peut
autoriser seul une transaction. C'est ce qu'on note `1-of-n`.

## Ce qui peut servir de signataire

Trois types, que vous pouvez combiner librement :

| Méthode | Ce que c'est | Exemple typique |
| --- | --- | --- |
| **Plateforme** | L'authentificateur intégré à l'appareil que vous utilisez | Face ID / Touch ID sur ce téléphone ou cet ordinateur portable, synchronisé par le trousseau iCloud ou le gestionnaire de mots de passe de Google |
| **Appareil à proximité** | Un autre appareil, que vous utilisez en scannant un code | Votre téléphone qui signe pour votre ordinateur, via le transport hybride de WebAuthn |
| **Clé de sécurité** | Un authentificateur amovible, en USB ou NFC | YubiKey et autres clés FIDO2 |

Tous trois sont des identifiants WebAuthn sur la courbe **P-256**. Pour le Safe,
ils sont indiscernables : chacun est un propriétaire dont la signature est vérifiée
on-chain de la même manière par le module passkey de Safe. (La première clé est
vérifiée par le signataire partagé de Safe ; chaque clé supplémentaire, par son
propre petit contrat signataire, créé par la fabrique de Safe la première fois que
le portefeuille est déployé sur une chaîne.)

Les types de clé que chaque app peut utiliser :

| App | Cet appareil | Téléphone à proximité (QR) | Clé de sécurité |
| --- | --- | --- | --- |
| Portefeuille web, extension de navigateur | Oui | Oui | USB ou NFC, via le navigateur |
| Bureau (macOS, Windows, Linux) | macOS et Windows | Oui | USB |
| Android | Oui (avec les services Google Play) | Oui | USB |
| iOS | Oui | Oui | YubiKey USB-C ou Lightning, firmware 5.8 ou plus récent |

L'option « cet appareil » de l'app de bureau (Touch ID, Windows Hello), et sa prise
en charge de Windows en général, sont récentes et moins testées que les autres
voies ; sur ordinateur, un téléphone ou une clé de sécurité reste aujourd'hui le
choix le plus fiable.

Une clé de sécurité peut être votre **premier** signataire, pas seulement une
sauvegarde. Si vous préférez que votre portefeuille ne dépende jamais d'un compte
Apple ou Google, créez-le avec **deux** clés de sécurité et gardez-en une en lieu
sûr. (Un portefeuille dont la seule clé n'est synchronisée nulle part ne peut pas
être créé : l'app demande une deuxième clé, car perdre cet unique appareil ferait
perdre le portefeuille.)

## Pourquoi ils sont choisis à la création

C'est la partie qui surprend, alors voici le mécanisme plutôt que des excuses.

L'adresse de votre portefeuille est **dérivée** de l'ensemble de ses
propriétaires. Vela la calcule avec `CREATE2` à partir des données de configuration
du Safe — qui incluent la clé publique de chaque signataire — avant que quoi que ce
soit ne soit déployé on-chain. C'est ce qui vous permet de recevoir des fonds à une
adresse qui n'existe pas encore.

Pour l'adresse, la conséquence est arithmétique : **un ensemble de clés différent,
c'est une adresse différente**. Ajouter un signataire plus tard n'étendrait pas
votre portefeuille ; cela calculerait un nouveau portefeuille, à une nouvelle
adresse, sans un centime de votre argent.

La question « puis-je ajouter une clé plus tard ? » a donc deux réponses honnêtes :

- **Avant d'y mettre des fonds** : oui — l'adresse n'est encore engagée à rien,
  alors recréez le portefeuille avec les clés que vous voulez.
- **Après y avoir mis des fonds** : l'adresse est là où se trouve votre argent. Le
  Safe lui-même peut changer de propriétaires sur une chaîne où votre portefeuille
  est déjà déployé — mais sur chaque chaîne où il ne l'est pas encore, la même
  adresse correspond toujours aux clés d'origine, si bien que les ensembles de
  propriétaires finiraient par diverger d'une chaîne à l'autre. Les garder
  synchronisés entre les chaînes est possible — certains portefeuilles intelligents
  le font —, mais Vela ne l'a pas développé, et ne propose donc pas de changement
  de propriétaires. Planifiez l'ensemble de clés dès la création.

## Ce contre quoi cela vous protège vraiment

**La perte d'un appareil.** Avec plus d'un signataire, un téléphone perdu n'est
qu'un désagrément : une autre clé signe. Avec un seul signataire et la
synchronisation du système désactivée, un téléphone perdu, c'est un portefeuille
perdu — c'est pourquoi « votre passkey se synchronise automatiquement » décrit un
réglage que vous contrôlez, pas une garantie que nous pouvons vous donner.

**Un compte de plateforme auquel vous ne faites plus confiance.** Si votre passkey
se trouve dans le trousseau iCloud ou le gestionnaire de mots de passe de Google,
celui qui contrôle ce compte peut potentiellement l'utiliser. Une clé de sécurité,
c'est vous qui l'avez en main, et elle ne se synchronise nulle part.

Et ce contre quoi cela ne vous protège **pas**, car le `1-of-n` joue dans les deux
sens : ajouter une deuxième clé ajoute une deuxième *entrée*, pas un second
verrou. Quiconque obtient l'un de vos signataires, quel qu'il soit, peut signer
seul. Plus de clés, c'est plus de résilience face à la perte et plus de surface
exposée au vol ; c'est le compromis, et c'est à vous de le faire.

## Si une clé est peut-être compromise

Une clé ne peut pas être retirée. Si l'une de vos clés est peut-être entre les
mains de quelqu'un d'autre — un téléphone non verrouillé qui a disparu, un code
que quelqu'un a vu, un compte Apple ou Google que vous ne contrôlez plus —,
**transférez tout vers un nouveau portefeuille** créé avec des clés de confiance.
L'ancienne adresse reste utilisable par cette clé sur tous les réseaux, y compris
pour les fonds que quiconque y enverrait plus tard.

## Récupérer ou ajouter

Ce sont deux choses différentes, et la documentation les sépare :

- [Récupération et connexion](/fr/docs/recovery) — retrouver un portefeuille
  existant sur un nouvel appareil avec une clé que vous avez déjà.
- Cette page — décider, dès le départ, quelles clés existent tout court.
