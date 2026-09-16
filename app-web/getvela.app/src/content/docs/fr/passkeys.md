---
title: Comment fonctionnent les passkeys
description: Le modèle de sécurité derrière Vela — ce qu'est une passkey, où vit votre clé, et pourquoi il n'y a rien à hameçonner.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Comment fonctionnent les passkeys

Tout le modèle de sécurité de Vela repose sur une idée : la clé qui contrôle votre
portefeuille est une **passkey**, créée par votre appareil et gardée par votre
système d'exploitation — aucune application, Vela compris, ne peut la lire — et
utilisée uniquement avec votre visage ou votre empreinte.

## Ce qu'est réellement une passkey

Une passkey est une paire de clés publique/privée créée par votre appareil. La
**clé privée** est gardée par le service de passkeys de votre système — en général
le trousseau iCloud chez Apple, le gestionnaire de mots de passe Google sur
Android — chiffrée de bout en bout, de sorte qu'aucune application ne peut la lire
ni la copier. Les applications n'obtiennent pas la clé ; elles obtiennent le droit
de *demander à votre appareil de signer quelque chose* après votre
authentification.

C'est la même technologie qui protège Apple Pay et votre déverrouillage
biométrique.

<Callout type="info" title="Le point essentiel">
Une application — Vela comprise — peut demander une signature, mais ne voit jamais
votre clé privée. Votre visage ou votre empreinte autorise votre appareil à
signer ; la clé elle-même reste dans votre système, chiffrée de bout en bout.
</Callout>

## Pourquoi il n'y a rien à hameçonner

L'hameçonnage fonctionne en vous faisant livrer un secret. Avec une phrase de
récupération, ce secret est douze mots que vous pouvez taper sur une fausse page.
Avec une passkey, **il n'y a aucun secret que l'on puisse taper**. Un site
frauduleux ne peut pas vous demander de « saisir votre passkey », parce qu'une
passkey ne se saisit pas — c'est une opération matérielle contrôlée par votre
biométrie.

Cela supprime la manière la plus courante de perdre des fonds auto-conservés.

## Ce que l'on ressent en signant

1. Vous confirmez une transaction dans Vela.
2. Votre appareil demande Face ID / Touch ID.
3. Votre appareil signe la transaction avec votre passkey.
4. Vela diffuse la transaction signée sur le réseau.

Le même geste que pour déverrouiller votre téléphone — parce que c'est le même
mécanisme de passkey que votre appareil utilise déjà partout ailleurs.

<Callout type="warning" title="La sécurité de l'appareil compte toujours">
Une passkey protège extrêmement bien contre les attaques à distance et
l'hameçonnage. Elle ne protège pas contre quelqu'un qui tient votre appareil
déverrouillé et qui passe votre contrôle biométrique. Gardez un code d'accès actif
et ne confiez pas un téléphone déverrouillé à quelqu'un en qui vous n'avez pas
confiance.
</Callout>

## Où vit le reste

La clé **publique** de votre passkey est publiée dans un petit index on-chain,
afin que votre portefeuille puisse être récupéré sur un nouvel appareil. C'est le
sujet de la page suivante : [Récupération et connexion](/fr/docs/recovery).
