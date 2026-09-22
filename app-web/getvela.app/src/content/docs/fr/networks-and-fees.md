---
title: Réseaux et frais
description: "Les 24 réseaux intégrés à Vela, comment en ajouter un autre, comment les frais d'une transaction sont calculés exactement et à qui ils reviennent, et ce qui se passe quand un relais n'a plus de gas."
source: fdc50dbbf13a
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Réseaux et frais

## Réseaux intégrés

Vela intègre **24 réseaux**, tous des mainnets :

| Réseau | Gas payé en | Réseau | Gas payé en |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (la monnaie native) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (la monnaie native) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (pas de monnaie native) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

Sur la plupart d'entre eux, vous pouvez aussi payer les frais avec un stablecoin en
dollars que le relais accepte sur ce réseau (voir plus bas).

Votre portefeuille a **la même adresse sur tous les réseaux**, parce que l'adresse
est calculée à partir de vos clés, et non à partir de la chaîne.

## Ajouter un autre réseau

Vous pouvez ajouter n'importe quel réseau EVM dans **Réglages → Réseaux**, à
condition qu'il dispose de ce dont un portefeuille Vela a besoin : onze contrats
standards (l'EntryPoint ERC-4337 v0.7, les contrats Safe v1.4.1, les modules 4337
et passkey de Safe, MultiSend, Multicall3 et deux déployeurs déterministes) et le
précompilé **RIP-7212**, qui vérifie les signatures de passkey à l'adresse `0x100`.
Le portefeuille vérifie tous ces éléments, y compris un vrai contrôle de signature
auprès du précompilé, avant de vous laisser ajouter le réseau.

Le précompilé est une exigence absolue. Son adresse entre dans le calcul de chaque
adresse Vela : il n'existe donc aucun vérificateur de repli, ni aucun moyen d'en
déployer un après coup. Si une chaîne dispose du précompilé mais qu'il lui manque
certains contrats, la page [Configurer une chaîne](/fr/chain-setup) indique ce qui
manque et déploie ce que tout le monde peut déployer. Une lacune dans cette
vérification : un portefeuille à plusieurs clés a aussi besoin de la fabrique de
signataires passkey de Safe sur le réseau, ce qui n'est pas encore vérifié ; sans
elle, seule la première clé peut signer sur ce réseau.

## Comment une transaction est payée

Vela est un portefeuille ERC-4337 : vous ne diffusez pas vous-même la transaction.
L'app construit une **UserOperation**, vous la signez avec l'une de vos clés, et un
**relais** la soumet on-chain en avançant le gas. (ERC-4337 appelle ce rôle un
bundler.) Le relais est remboursé **à l'intérieur de votre opération** : le
paiement est un transfert de votre portefeuille vers le relais, placé dans le même
lot que votre transaction, et donc couvert par votre signature. Il n'y a pas de
paymaster : personne ne sponsorise votre gas, et personne ne peut refuser votre
transaction au nom d'une politique de sponsoring.

### Le montant des frais

<span id="fee"></span>

L'écran de confirmation affiche un seul montant, dans la monnaie des frais et dans
votre devise d'affichage. Il est calculé ainsi :

- **Le gas que le portefeuille réserve.** Le portefeuille simule la transaction et
  réserve plus de gas qu'il ne compte en utiliser : les estimations de vérification
  et d'exécution sont chacune majorées de moitié, avec des minimums (par exemple,
  au moins 300 000 gas pour la vérification une fois le portefeuille déployé, et
  2 000 000 pour la transaction qui le déploie).
- **Le prix du gas.** Le plus élevé entre le prix du gas du réseau relevé par le
  portefeuille lui-même et le prix du relais pour la vitesse choisie. La vitesse
  par défaut est *rapide*, que le relais facture environ 1,8 × les frais de base
  plus deux fois les frais de priorité.
- **Frais = 3 × gas réservé × prix du gas**, avec un minimum d'environ 0,01 $. Sur
  Tempo, le multiplicateur est de 2 et les frais se paient en pathUSD.

La réserve est largement supérieure à ce que la transaction consommera et le prix
comporte une marge : les frais dépassent donc ce que la transaction coûte on-chain —
et plus encore pour votre première transaction sur un réseau, qui déploie aussi
votre portefeuille. Le relais paie le coût réel et garde le reste ; rien n'est
remboursé. Sur les réseaux bon marché, cela se compte en centimes ; sur le mainnet
Ethereum, cela peut représenter une vraie somme. Inutile de deviner : le montant
exact figure sur l'écran de confirmation avant que vous signiez.

**Qui les reçoit.** Les frais reviennent à celui qui exploite le relais configuré
dans le portefeuille — celui de Vela, sauf si vous en changez. N'importe quel
déploiement de vela-relay convient, y compris
[celui que vous faites tourner vous-même](/fr/docs/self-hosting#relay), et le
portefeuille applique la même formule quel que soit le relais choisi.

<Callout type="info" title="Ce que vous voyez est ce que vous payez">
Le montant des frais et l'adresse qui les reçoit font partie de l'opération que
vous signez. Un relais qui modifierait l'un ou l'autre invaliderait votre
signature : vous payez donc exactement le montant affiché — pas plus, même si le
gas augmente avant l'inclusion. Un prix du gas proposé par un relais qui dépasse
trois fois le relevé du portefeuille est refusé.
</Callout>

### Avec quoi payer

- La **monnaie native** du réseau, toujours.
- Un **stablecoin en dollars** figurant sur la liste du relais pour ce réseau,
  quand le relais peut donner un prix à la monnaie native. Les stablecoins que vous
  ne détenez pas sont masqués.
- Sur **Tempo**, qui n'a pas de monnaie native, uniquement le **pathUSD**.

Vous choisissez la monnaie des frais, et la vitesse (*lente*, *standard* ou
*rapide*), sur l'écran de confirmation et dans les Réglages.

### Votre première transaction sur un réseau

Vous pouvez recevoir sur n'importe quel réseau avant que votre portefeuille y
existe. La première fois que vous envoyez depuis un réseau, cette transaction
déploie aussi le contrat de votre portefeuille (et un petit contrat signataire pour
chaque clé supplémentaire). Le gas du déploiement est inclus dans les frais de
cette transaction : le premier envoi sur chaque réseau coûte donc plus cher que les
suivants.

Quand vous envoyez le **maximum** d'une monnaie native, Vela garde de côté de quoi
payer les frais.

## Qui exploite le relais — et à qui reviennent les frais

Par défaut, tous les réseaux utilisent **le relais de Vela**, et les frais
reviennent à Vela. Vous pouvez faire pointer le portefeuille vers un autre relais
dans **Réglages → Avancé → Points d'accès des services** ; une seule adresse sert
tous les réseaux intégrés, et un réseau personnalisé garde l'adresse de relais avec
laquelle il a été ajouté. Le relais doit être
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — celui de Vela ou un que
vous faites tourner —, car le portefeuille lui demande son devis de frais avec une
méthode propre à Vela que les bundlers génériques, comme Pimlico ou Alchemy,
n'implémentent pas. C'est l'exploitant du relais que vous utilisez qui reçoit les
frais ; le [guide d'auto-hébergement](/fr/docs/self-hosting#relay) explique comment
faire tourner le vôtre.

Le relais reçoit une opération déjà signée. Il ne peut modifier ni le
destinataire, ni le montant, ni les frais, ni quoi que ce soit d'autre. Il peut la
retarder ou la refuser, et il choisit le moment où elle est incluse — pour un swap,
il pourrait donc, en principe, passer un ordre avant le vôtre dans la limite de
votre slippage.

### Quand un relais n'a plus de gas

Un relais paie le gas depuis sa propre **trésorerie** sur chaque réseau. Si cette
trésorerie est vide, l'écran d'envoi vous le dit avant que vous signiez :

- Sur un réseau servi par le relais de Vela, c'est l'exploitant du relais (Vela)
  qui doit la réapprovisionner ; vous pouvez signaler le problème. Si vous ne
  pouvez pas attendre, vous pouvez **si vous le souhaitez** envoyer vous-même un
  petit montant de monnaie native à la trésorerie. Cette contribution est **non
  remboursable** et ne paie **pas** votre propre transaction.
- Sur un réseau personnalisé, approvisionner le relais revient à celui qui
  l'exploite — ce qui peut être vous.

Il n'y a ni compte de gas par portefeuille, ni dépôt d'activation : une ancienne
version de Vela en avait un, qui n'existe plus.

## Comment Vela lit chaque réseau

Vela lit les soldes et simule les transactions via un **ensemble de points d'accès
RPC** par réseau — ceux intégrés, des solutions de repli publiques, et les clés de
fournisseur ou points d'accès que vous ajoutez — et passe au suivant quand un point
d'accès est lent ou indisponible. Vous pouvez définir votre propre point d'accès
pour chaque réseau dans **Réglages → Réseaux**. (L'app Android utilise pour
l'instant un seul point d'accès par réseau, sans bascule, et l'app iPhone ne permet
pas encore de le modifier.)

Ensuite : [comment fonctionnent les passkeys](/fr/docs/passkeys).
