---
title: Signataires et clés de sécurité
description: Un portefeuille Vela peut avoir jusqu'à sept signataires — passkeys, un appareil à proximité ou une clé de sécurité de type YubiKey — et n'importe lequel signe seul. Ils sont choisis à la création du portefeuille, et cette page explique pourquoi ce n'est pas une limitation qu'on aurait oublié de lever.
---

# Signataires et clés de sécurité

Un portefeuille Vela est un Safe, et un Safe a des propriétaires. Le vôtre peut en
avoir **jusqu'à sept**, avec un seuil de **un** : n'importe quel signataire peut
autoriser une transaction à lui seul. Cela s'écrit `1-of-n`.

## Ce qui peut être un signataire

Trois formes, librement mélangeables :

| Méthode | Ce que c'est | Exemple typique |
| --- | --- | --- |
| **Plateforme** | L'authentificateur intégré à l'appareil que vous utilisez | Face ID / Touch ID sur ce téléphone ou cet ordinateur, synchronisé par le trousseau iCloud ou le gestionnaire de mots de passe Google |
| **Appareil à proximité** | Un autre appareil que vous rejoignez en scannant un code | Votre téléphone signe pour votre ordinateur, via le transport hybride de WebAuthn |
| **Clé de sécurité** | Un authentificateur amovible en USB ou NFC | YubiKey et autres clés FIDO2 |

Les trois sont des identifiants WebAuthn sur la courbe **P-256**. Pour le Safe,
elles sont indiscernables : chacune est un propriétaire dont le vérificateur
WebAuthn on-chain contrôle la signature de la même façon.

Une clé de sécurité peut être votre **premier** signataire, pas seulement une
sauvegarde. Si vous préférez que votre portefeuille ne dépende jamais d'un compte
Apple ou Google, c'est le réglage qui le permet : enregistrez une YubiKey à la
création et signez avec.

## Pourquoi ils sont choisis à la création

C'est la partie qui surprend, alors voici le mécanisme plutôt qu'une excuse.

L'adresse de votre portefeuille est **dérivée** de son ensemble de propriétaires.
Vela la calcule avec `CREATE2` à partir des données de configuration du Safe — qui
incluent la clé publique de chaque signataire — avant que quoi que ce soit ne soit
déployé on-chain. C'est précisément ce qui vous permet de recevoir à une adresse
qui n'existe pas encore.

La conséquence relève de l'arithmétique, pas de la politique : **un ensemble de
clés différent, c'est une adresse différente**. Ajouter un huitième signataire
plus tard n'étendrait pas votre portefeuille ; cela calculerait un nouveau
portefeuille, à une nouvelle adresse, sans un centime de votre argent dedans.

La question « puis-je ajouter une clé plus tard ? » a donc deux réponses
honnêtes :

- **Avant de l'alimenter :** oui — l'adresse ne s'est engagée sur rien, recréez le
  portefeuille avec les clés que vous voulez.
- **Après l'avoir alimenté :** l'adresse est là où se trouve votre argent. Changer
  les propriétaires d'un Safe déployé est une opération Safe que Vela n'expose pas
  aujourd'hui. Planifiez l'ensemble de clés à la création.

## Ce dont cela vous protège vraiment

**Perdre un appareil.** Avec plus d'un signataire, un téléphone perdu est un
désagrément : une autre clé signe. Avec exactement un signataire et la
synchronisation du système désactivée, un téléphone perdu est un portefeuille
perdu — c'est pourquoi « votre passkey se synchronise automatiquement » décrit un
réglage que vous contrôlez, et non une garantie que nous pourrions donner à votre
place.

**Un compte de plateforme auquel vous ne faites plus confiance.** Si votre passkey
vit dans le trousseau iCloud ou le gestionnaire de mots de passe Google, qui
contrôle ce compte peut potentiellement s'en servir. Une clé de sécurité, vous la
gardez, et elle ne se synchronise nulle part.

Et ce dont cela ne vous protège **pas**, parce que `1-of-n` coupe des deux côtés :
ajouter une deuxième clé ajoute une deuxième *entrée*, pas un deuxième verrou.
Quiconque obtient l'un de vos signataires peut signer seul. Plus de clés, c'est
plus de résistance à la perte et plus de surface au vol ; c'est l'arbitrage, et
c'est le vôtre.

## Récupérer n'est pas ajouter

Ce sont deux choses différentes, et la documentation les tient séparées :

- [Récupération et connexion](/fr/docs/recovery) — revenir à un portefeuille
  existant depuis un nouvel appareil, avec une clé que vous avez déjà.
- Cette page — décider, en amont, quelles clés existent tout court.
