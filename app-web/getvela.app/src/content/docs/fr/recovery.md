---
title: Récupération et connexion
description: "Comment revenir dans votre portefeuille sur un nouvel appareil avec n'importe laquelle de vos clés, où le portefeuille est recherché, et les limites réelles d'une récupération sans phrase de récupération."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Récupération et connexion

Sans phrase de récupération, la récupération repose sur deux choses : **une clé que
vous avez encore**, et **un enregistrement public des clés qui appartiennent à
votre portefeuille**.

## Ce qui est enregistré à la création d'un portefeuille

L'adresse de votre portefeuille est calculée à partir de toutes les clés avec
lesquelles vous le créez. Pour que n'importe laquelle de ces clés puisse retrouver
le portefeuille plus tard, créer un portefeuille inscrit un enregistrement dans un
**contrat de registre** public sur Gnosis Chain : la clé publique de chaque clé,
l'adresse du portefeuille, son nom, et les données d'enregistrement signées. Le
registre n'a pas de propriétaire et ne peut être ni modifié ni effacé. (La liste
complète de ce qui est public se trouve sur
[créer votre portefeuille](/fr/docs/create-wallet#what-is-public).)

Le service d'index des clés publiques de Vela soumet cet enregistrement et en paie
le gas ; l'enregistrement lui-même est on-chain, et n'importe quelle app peut le
lire directement.

## Se connecter sur un nouvel appareil

1. Ouvrez Vela et choisissez de vous connecter.
2. Utilisez **n'importe laquelle** de vos clés : une passkey synchronisée sur cet
   appareil, un téléphone à proximité (scannez le QR code) ou votre clé de
   sécurité.
3. Vela déduit la clé publique de cette clé à partir de la signature, la recherche —
   d'abord dans l'index de Vela, puis, si l'index ne répond pas, dans le contrat de
   registre sur Gnosis, puis sur Ethereum — et reconstruit le portefeuille. Avant de
   vous l'afficher, il vérifie que les clés trouvées aboutissent bien à l'adresse
   enregistrée.

Les listes de comptes ne sont pas synchronisées entre appareils ; la connexion les
reconstruit.

<Callout type="info" title="Si aucun index ni registre ne répond">
Un portefeuille à <strong>clé unique</strong> peut être reconstruit sur l'appareil
sans aucun serveur : deux signatures de cette clé suffisent pour retrouver sa clé
publique et recalculer l'adresse. Un portefeuille à plusieurs clés a besoin de
l'enregistrement du registre, car une clé ne peut pas dire à l'app quelles étaient
les autres.
</Callout>

## Copies de l'enregistrement

Le registre sur Gnosis est celui que les apps lisent en premier. Depuis les
**Réglages**, vous pouvez aussi copier l'enregistrement de votre portefeuille dans
le même contrat de registre sur **Ethereum**, en payant vous-même le gas, pour que
l'enregistrement existe sur une deuxième chaîne. N'importe qui peut faire une telle
copie ; elle ne contient rien qui puisse déplacer des fonds.

## Les limites, honnêtement

<Callout type="warning" title="Une clé perdue est perdue">
Si toutes les clés avec lesquelles vous avez créé le portefeuille ont disparu — les
passkeys synchronisées, les téléphones, les clés de sécurité —, personne ne peut
récupérer le portefeuille : ni Vela, ni Apple, ni Google, ni personne. Il n'y a ni
phrase de récupération, ni réinitialisation par le support, ni porte dérobée.
</Callout>

Ce qui rend ce scénario improbable, c'est d'avoir plus d'un moyen d'entrer :

- **Laissez la synchronisation des passkeys activée** si vous utilisez la passkey
  de cet appareil. C'est elle qui transporte la clé vers un nouveau téléphone ou un
  nouvel ordinateur.
- **Sécurisez le compte qui se trouve derrière.** Celui qui contrôle votre compte
  Apple ou Google peut être en mesure d'utiliser une passkey synchronisée ;
  donnez-lui un mot de passe solide et ses propres options de récupération.
- **Créez le portefeuille avec plus d'une clé**, par exemple la passkey de votre
  téléphone et une clé de sécurité matérielle gardée en lieu sûr. Les clés ne
  peuvent être ajoutées qu'à la création du portefeuille ([pourquoi](/fr/docs/signers)).
  N'oubliez pas que n'importe quelle clé peut signer seule — et ne peut pas être
  retirée : si l'une d'elles est un jour compromise, transférez vos fonds vers un
  nouveau portefeuille ([que faire](/fr/docs/signers)).

## Ce que Vela peut faire et ne peut pas faire

- **Peut :** faire fonctionner l'index, pour que votre portefeuille soit retrouvé
  rapidement sur un nouvel appareil.
- **Ne peut pas :** déplacer vos fonds, geler votre portefeuille, ajouter ou
  retirer des clés, ni récupérer une clé que vous avez perdue. Vela ne détient
  jamais vos clés.

Ensuite : [la signature lisible](/fr/docs/clear-signing).
