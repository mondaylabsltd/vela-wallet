---
title: Récupération et connexion
description: Comment Vela vous laisse récupérer votre portefeuille sur un nouvel appareil sans phrase de récupération — et les limites honnêtes de ce modèle.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Récupération et connexion

La partie la plus difficile d'un portefeuille sans phrase de récupération, c'est
justement la récupération : s'il n'y a pas douze mots, comment revenir depuis un
nouveau téléphone ? Voici exactement comment Vela s'y prend.

## Comment ça marche

Quand vous créez un portefeuille, deux choses sont publiées dans l'**index de
passkeys** de Vela :

- La **clé publique** de votre passkey (jamais la clé privée).
- Le **nom** que vous avez choisi pour le portefeuille.

La clé publique est stockée sur la blockchain Gnosis via un contrat : elle est
donc lisible publiquement et ne dépend pas du fait que les serveurs de Vela
restent en ligne.

Votre clé **privée**, elle, est une passkey synchronisée par le trousseau de votre
plateforme — **trousseau iCloud** sur les appareils Apple, **gestionnaire de mots
de passe Google** sur Android.

Pour vous connecter sur un nouvel appareil :

1. Connectez-vous au même compte iCloud ou Google, avec la synchronisation du
   trousseau activée.
2. Ouvrez Vela et choisissez de vous connecter.
3. Authentifiez-vous avec votre passkey. Votre plateforme fournit la passkey
   synchronisée ; l'index fournit le compte correspondant. Votre portefeuille est
   de retour.

L'index est un cache, pas un point de défaillance unique. S'il devenait
inaccessible et que votre compte n'est pas en stockage local, Vela peut
reconstruire votre clé publique sur l'appareil à partir de deux signatures de
passkey, puis en redériver l'adresse de votre portefeuille — sans aucun serveur.

<Callout type="info" title="Pourquoi séparer ainsi">
La clé publique dans l'index on-chain permet à n'importe qui (y compris une
installation toute neuve) de retrouver votre compte. La clé privée, synchronisée
par le trousseau de la plateforme en qui vous avez confiance, est ce qui autorise
réellement les transactions. Tout ce qui est dans l'index est public, et rien de
ce qui s'y trouve ne peut déplacer vos fonds — seules les signatures de votre
passkey le peuvent.
</Callout>

## Les limites, honnêtement

L'auto-conservation veut dire que la responsabilité est réelle. Voici ce qu'il
faut comprendre.

<Callout type="warning" title="Votre récupération dépend du trousseau de votre plateforme">
La connexion multi-appareils de Vela repose sur la synchronisation de votre
passkey via le trousseau iCloud ou le gestionnaire de mots de passe Google. Gardez
ce compte sécurisé et ses options de récupération à jour. Si vous perdez
<strong>à la fois</strong> vos appareils <strong>et</strong> le trousseau de votre
compte de plateforme, Vela ne peut pas régénérer votre clé privée — par
conception, nous ne l'avons jamais eue.
</Callout>

Conseils pratiques :

- **Laissez la synchronisation du trousseau active.** C'est elle qui transporte
  votre passkey d'un appareil à l'autre.
- **Sécurisez votre compte Apple / Google** avec un mot de passe solide et ses
  propres méthodes de récupération. Ce compte fait désormais partie de la sécurité
  de votre portefeuille.
- **Gardez plus d'un appareil connecté** quand c'est possible, pour qu'un
  téléphone perdu reste un désagrément et non une crise.

## Ce que Vela peut et ne peut pas faire

- **Peut :** vous aider à retrouver votre compte grâce à l'index public.
- **Ne peut pas :** déplacer vos fonds, geler votre portefeuille ou récupérer une
  clé privée. Vela ne l'a jamais détenue. C'est tout l'intérêt de
  l'auto-conservation — et le marché que vous passez pour l'obtenir.
