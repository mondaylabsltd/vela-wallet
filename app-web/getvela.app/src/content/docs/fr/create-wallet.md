---
title: Créer votre portefeuille
description: Créez un portefeuille Vela auto-conservé en une minute environ avec une passkey — sans phrase de récupération. Votre portefeuille est un compte intelligent Safe, avec la même adresse sur chaque réseau.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Créer votre portefeuille

Créer un portefeuille prend environ une minute et une seule vérification
biométrique. Ouvrez le portefeuille web sur
[wallet.getvela.app](https://wallet.getvela.app/) et choisissez **Créer un
portefeuille**.

## Les étapes

1. **Nommez votre portefeuille.** Choisissez un nom qui vous permettra de
   reconnaître le compte plus tard, y compris en vous connectant depuis un autre
   appareil. Il est stocké à côté de votre clé publique : traitez-le comme public
   et n'y mettez rien de personnel.
2. **Confirmez les bases.** Une courte liste confirme que vous avez compris que
   Vela est auto-conservé et encore en alpha, avec les liens vers la
   [politique de confidentialité](/privacy) et les
   [conditions d'utilisation](/terms).
3. **Créez votre passkey.** À l'invite, authentifiez-vous avec **Face ID, Touch ID
   ou votre empreinte**. Cela crée une passkey WebAuthn (P-256) que votre appareil
   garde et que Vela ne voit jamais. Il n'y a pas d'étape « notez la phrase de
   récupération », parce qu'il n'y a pas de phrase de récupération.
4. **C'est fait.** Vela affiche l'adresse de votre portefeuille et vous êtes
   dedans. Vous pouvez la vérifier, puis vous connecter pour arriver dans votre
   portefeuille.

## Ce qu'est réellement votre portefeuille

C'est la partie que la plupart des portefeuilles n'expliquent pas — et elle
détermine le fonctionnement de Vela.

Votre portefeuille Vela est un **compte intelligent Safe** (un contrat), et non un
simple « compte détenu en externe ». Votre passkey est le propriétaire de ce
compte ; un montage ERC-4337 vous permet de l'opérer avec votre seul visage ou
votre seule empreinte.

<Callout type="info" title="Votre adresse est la même sur chaque réseau">
Vela dérive votre adresse de la clé publique de votre passkey : elle est
identique sur Ethereum, Base, Arbitrum, Gnosis et tous les autres réseaux pris en
charge. Vous ne donnez qu'une seule adresse, partout.
</Callout>

Conséquence utile : l'adresse est **contrefactuelle**. Elle est calculée avant que
quoi que ce soit ne soit déployé on-chain, donc **vous pouvez recevoir des fonds
avant même que le contrat de votre portefeuille existe**. Le contrat se déploie
tout seul, payé sur son propre solde, à votre première transaction sur un réseau
donné.

## Ce qui vient d'arriver à vos clés

- Votre appareil a généré une **paire de clés de passkey**.
- La **clé privée** est gardée par le service de passkeys de votre système
  (trousseau iCloud ou gestionnaire de mots de passe Google), chiffrée de bout en
  bout et synchronisée entre vos appareils — aucune application, Vela compris, ne
  la voit jamais.
- La **clé publique et le nom choisi** sont publiés dans l'index de passkeys de
  Vela, qui écrit aussi la clé dans un registre publiquement lisible sur Gnosis
  Chain, afin que votre compte puisse être retrouvé depuis un nouvel appareil.
  Voir [Récupération et connexion](/fr/docs/recovery).

## Prochaines étapes

- [Recevoir vos premiers tokens](/fr/docs/send-and-receive)
- [Comprendre les réseaux et les frais](/fr/docs/networks-and-fees)
- [Lire pourquoi les passkeys rendent cela sûr](/fr/docs/passkeys)
