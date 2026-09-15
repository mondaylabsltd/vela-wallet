---
title: Signature lisible
description: Vela décode les transactions en langage clair avant que vous ne les approuviez — intention, montants, adresses et risque — plutôt qu'en hexadécimal opaque. Quand il ne peut pas décoder un appel, il vous prévient au lieu de faire semblant.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Signature lisible

La plupart des portefeuilles vous demandent d'approuver un mur d'hexadécimal et
d'espérer que tout se passe bien. La « signature à l'aveugle » — approuver des
appels que vous ne pouvez pas lire — est derrière une grande partie des
portefeuilles vidés. La réponse de Vela, c'est la **signature lisible** : avant
que vous signiez, la transaction est traduite en quelque chose de compréhensible.

## Ce que vous voyez

À la place de la calldata brute, Vela affiche :

- **L'intention** — ce que fait la transaction : *Envoyer*, *Approuver*,
  *Échanger*, etc.
- **La substance** — les montants et les adresses concernés, les montants de
  tokens en unités réelles et les destinataires résolus en un nom quand il en
  existe un.
- **Les détails** — nonce, échéances et calldata brute, disponibles à la demande
  plutôt qu'imposés d'emblée.
- **Une indication de risque**, en couleurs, pour que ce qui fait peur en ait
  l'air.

## Comment ça marche (ERC-7730)

Vela décode à la fois les **appels de contrat** et les **données typées EIP-712**
à l'aide de descripteurs
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry) — de
petites définitions partageables de ce que signifient les fonctions d'un contrat.

- Quand un **descripteur propre au contrat** existe, la transaction est marquée
  **vérifiée** et porte le nom du contrat.
- Sinon, Vela retombe sur des **descripteurs standards** pour les formes
  courantes — tokens ERC-20, NFT ERC-721, coffres ERC-4626 et permits ERC-2612 —
  de sorte que la plupart des actions quotidiennes se décodent quand même.

Les montants de tokens sont formatés avec les **décimales réelles on-chain** du
token. Vela ne suppose jamais 18 ; s'il ne peut pas confirmer les décimales, il
affiche la valeur mais **la signale comme non vérifiée** plutôt que de deviner.

## Niveaux de risque

Chaque transaction décodée reçoit un niveau de risque, pour que les schémas
dangereux ressortent :

- **Prudence** pour les approbations et les permits — vous accordez un pouvoir de
  dépense.
- **Danger** pour ce qui est vraiment risqué, comme une **approbation de token
  illimitée**.
- Risque moindre pour les actions de routine comme le staking ou un dépôt.

<Callout type="warning" title="Les approbations illimitées sont bloquées">
Un « approve » qui accorde une allocation illimitée est l'une des manières les plus
courantes de voir des fonds disparaître plus tard. Vela ne se contente pas de les
signaler : il réécrit la demande en un montant fini que vous choisissez, et une
dernière vérification avant l'envoi refuse toute approbation qui resterait
illimitée. Ce garde-fou lit directement la calldata brute : il fonctionne même
quand aucun descripteur n'existe pour le contrat.
</Callout>

## Quand Vela ne peut pas décoder un appel

L'honnêteté compte plus qu'un écran propre. Quand aucun descripteur ERC-7730
n'existe mais que la fonction apparaît dans une base publique de sélecteurs, Vela
décode l'appel de façon générique et le marque **au mieux** — décodé, mais non
vérifié — sous une bannière de prudence. Si même cela échoue, ou si Vela ne peut
décoder qu'une partie d'une transaction, il ne fait **pas** semblant de
comprendre.

<Callout type="danger" title="Avertissement explicite de signature à l'aveugle">
Si un appel ne peut pas être décodé, Vela affiche un avertissement clair de
signature à l'aveugle au lieu d'un résumé faussement rassurant. S'il ne peut
résoudre qu'une partie des champs, il vous dit que la vue est partielle et
maintient le niveau de risque élevé. Vous savez toujours quelle part de ce que
vous signez Vela a réellement pu lire.
</Callout>

## Pourquoi c'est important

L'auto-conservation signifie que personne ne peut annuler une mauvaise
transaction à votre place. La défense n'est pas un service client — c'est de
comprendre ce que vous approuvez **avant** de l'approuver. La signature lisible
transforme « faites confiance à ce bloc opaque » en « voici exactement ce que
cela fait ». Sa place dans le modèle de sécurité global est dans le
[livre blanc](/fr/docs/whitepaper).
