---
title: Signature lisible
description: "Vela décode les transactions en langage clair avant que vous ne les approuviez — intention, montants, adresses et risque — plutôt qu'en hexadécimal opaque. Quand il ne parvient pas à décoder un appel, il vous prévient au lieu de faire semblant."
source: 7232328b724e
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Signature lisible

Beaucoup de portefeuilles affichent encore des données brutes pour tout contrat
qu'ils ne reconnaissent pas, et la « signature à l'aveugle » — approuver des appels
que l'on ne peut pas vraiment lire — est l'une des façons dont les portefeuilles se
font vider. La réponse de Vela, c'est la **signature lisible** : avant que vous
signiez, la transaction est décodée, autant que possible, en quelque chose que vous
pouvez comprendre.

## Ce que vous voyez

À la place de la calldata brute, Vela affiche :

- **L'intention** — ce que fait la transaction : *Envoyer*, *Approuver*,
  *Échanger*, etc.
- **L'essentiel** — les montants et les adresses concernés, avec les montants de
  jetons en unités réelles et les destinataires affichés par leur nom quand il en
  existe un.
- **Les détails** — nonce, échéances et calldata brute, disponibles à la demande
  plutôt que jetés à la figure.
- **Une indication de risque**, par code couleur, pour que les actions dangereuses
  ressortent.

## Comment ça marche (ERC-7730)

Vela décode à la fois les **appels de contrat** et les **données typées EIP-712**
à l'aide de descripteurs
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry) — de petites
définitions partageables de ce que signifient les fonctions d'un contrat.

Vela cherche un descripteur dans cet ordre :

1. **Intégré à l'app** — des descripteurs pour des contrats très utilisés : les
   routeurs Uniswap, PancakeSwap et SushiSwap, WETH, le pool Aave v3, 1inch, Lido
   et wstETH, et Seaport.
2. **Récupéré sur le serveur de données de chaîne de Vela**, qui republie le
   registre public ERC-7730.
3. **Formes standards** — jetons ERC-20, NFT ERC-721 et ERC-1155, coffres ERC-4626
   et permits ERC-2612 —, pour que la plupart des actions courantes se décodent
   quand même.

**Vérifiée** est réservé à la première source. Une transaction n'est marquée
vérifiée que si la description vient d'un descripteur intégré à l'app que vous
utilisez — ou du serveur de données de chaîne et qu'elle est identique à la copie
intégrée, ce qui prouve que rien n'a été modifié en chemin. Tout le reste de ce que
le serveur envoie est quand même décodé et quand même affiché, avec une ligne
indiquant que cela vient du service de descripteurs et que rien ne l'a authentifié.
Ce service n'est pas signé : il n'est donc fiable qu'autant que celui qui le fait
tourner — c'est l'une des raisons pour lesquelles vous pouvez
[faire tourner le vôtre](/fr/docs/self-hosting#chain-data).

Les montants de jetons sont formatés avec les **décimales réelles on-chain** du
jeton. Si Vela ne peut pas confirmer les décimales d'un jeton, il affiche le
montant comme si le jeton en avait 18 et **le signale comme non vérifié**, pour
qu'un chiffre faux n'ait jamais l'air d'un chiffre contrôlé.

## Niveaux de risque

Chaque transaction décodée reçoit un niveau de risque, pour que les schémas
dangereux ressortent :

- **Prudence** pour les approbations et les permits — vous accordez un pouvoir de
  dépense.
- **Danger** pour ce qui est vraiment risqué, comme une **approbation de jeton
  illimitée**.
- Un risque moindre pour les actions de routine comme le staking ou un dépôt.

<Callout type="warning" title="Les approbations « illimitées » on-chain ne peuvent pas être soumises">
Une approbation on-chain d'un montant illimité est l'une des façons les plus
courantes de voir des fonds disparaître plus tard. Quand une dApp en demande une
(<code>approve</code>, <code>increaseAllowance</code> ou l'<code>approve</code> de
Permit2) au niveau « illimité » — 2^200 ou plus (2^152 pour Permit2), les valeurs
qu'utilisent les dApps pour dire « illimité » —, Vela ne la soumet pas tant que vous
ne l'avez pas changée en un montant précis, votre solde ou une révocation ; une
dernière vérification avant l'envoi lit la calldata brute, si bien qu'elle
fonctionne avec ou sans descripteur. Ce qu'elle n'arrête pas : une
<strong>approbation finie mais élevée</strong> (même bien au-delà de votre solde),
les <strong>permits signés</strong> (signatures EIP-2612 et Permit2) et le
<code>setApprovalForAll</code> des NFT — chacun s'affiche avec un avertissement, et
la décision vous appartient.
</Callout>

## Quand Vela ne parvient pas à décoder un appel

Quand aucun descripteur ERC-7730 n'existe mais que la fonction figure dans une base
publique de sélecteurs, Vela décode l'appel de façon générique et le marque **au
mieux** — décodé, mais non vérifié — sous une bannière de prudence. Si même cela
échoue, ou si Vela ne peut décoder qu'une partie d'une transaction, il ne fait
**pas** semblant de la comprendre.

<Callout type="danger" title="Avertissement explicite de signature à l'aveugle">
Si un appel ne peut pas être décodé, Vela affiche un avertissement clair de
signature à l'aveugle au lieu d'un résumé faussement rassurant. S'il ne peut
résoudre qu'une partie des champs, il vous dit que l'affichage est partiel et
maintient un niveau de risque élevé. Vous savez toujours quelle part de ce que vous
signez Vela a réellement pu lire.
</Callout>

## Pourquoi c'est important

L'auto-conservation signifie que personne ne peut annuler une mauvaise transaction
à votre place. La défense, ce n'est pas un service client — c'est de comprendre ce
que vous approuvez **avant** de l'approuver. La signature lisible, c'est la façon
dont Vela essaie de vous le montrer, et elle a ses limites : elle ne peut être plus
honnête que l'app qui l'affiche, et c'est pourquoi une
[vérification indépendante](/fr/docs/clear-signing-self-host) compte. Voir le
[livre blanc](/fr/docs/whitepaper) pour sa place dans le modèle de sécurité de Vela.
