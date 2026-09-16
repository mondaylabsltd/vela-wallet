---
title: Réseaux et frais
description: Les 12 réseaux pris en charge par Vela, le fonctionnement des frais de gas avec l'abstraction de compte, qui opère le relais et encaisse les frais, quand vous payez vous-même l'activation du compte de gas, et comment Vela choisit ses points de terminaison RPC.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Réseaux et frais

## Réseaux pris en charge

Vela intègre **12 réseaux EVM** :

| Réseau | Token natif des frais |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Votre portefeuille a **la même adresse sur tous**, il n'y a donc qu'une adresse à
partager partout.

Vous pouvez aussi **ajouter des réseaux personnalisés** (Réglages → Réseaux).
Comme Vela est un portefeuille à compte intelligent, un réseau doit fournir les
contrats dont Vela dépend — l'EntryPoint ERC-4337, les contrats Safe et le
précompilé de signature **P-256 (RIP-7212)** qui vérifie votre passkey on-chain.
Vela le vérifie automatiquement avant de vous laisser ajouter un réseau.

<Callout type="info" title="Pourquoi Gnosis revient souvent">
Au-delà d'être l'un des 12 réseaux, Gnosis Chain héberge l'**index de passkeys**
de Vela — le contrat qui stocke votre clé publique et le nom de votre compte pour
la récupération multi-appareils. C'est indépendant du réseau sur lequel vous
transigez.
</Callout>

## Comment fonctionnent les frais (abstraction de compte)

Vela utilise l'**abstraction de compte ERC-4337** : une transaction n'est pas
diffusée par vous directement, c'est une **UserOperation** confiée à un **relais**,
qui la soumet on-chain et se fait rembourser le gas. (La spécification ERC-4337
appelle ce rôle un *bundler*. Celui de Vela s'appelle un relais parce qu'il fait
plus que grouper : il annonce les frais en ligne et opère le protocole de compte de
gas décrit plus bas, deux choses hors standard.) Plusieurs conséquences en
découlent :

- **Le gas est payé sur le solde de votre propre portefeuille** — dans le token
  natif du réseau (ETH, BNB, xDAI…) par défaut, ou dans un stablecoin pris en
  charge là où le relais en propose un ; vous choisissez l'actif de frais sur
  l'écran de confirmation. Tempo n'a pas de pièce native : le gas y est toujours
  réglé en stablecoins USD. Il n'y a pas de **paymaster** ERC-4337 qui sponsorise —
  ou conditionne — chaque transaction. (Vela peut prendre en charge l'unique
  _activation du compte de gas_ pour les nouveaux utilisateurs ; c'est autre chose,
  détaillé plus bas.)
- **C'est le relais qui annonce le prix du gas** — il est la source unique de
  vérité, et le portefeuille affiche cette annonce et signe exactement ce qu'il
  affiche. Il n'y a pas de sélecteur de vitesse : chaque transaction est soumise en
  priorité haute.
- Le total, ce sont les **coûts réseau plus les frais de service du relais**, avec
  un petit minimum sur les transactions très bon marché. L'annonce du relais est le
  prix — il n'y a pas de grille tarifaire séparée à consulter. Une part va aux
  validateurs de la chaîne ; le reste rémunère le relais qui avance le gas et fait
  tourner l'infrastructure.
- L'écran de confirmation affiche les **frais estimés** dans l'actif de frais et
  dans votre devise d'affichage avant que vous signiez. Le montant annoncé et son
  destinataire font partie de ce que vous signez : le relais est payé exactement ce
  qui était affiché — un chiffre modifié invaliderait votre signature.

## Qui opère le relais — et qui touche les frais

Chaque réseau pointe vers un relais. Par défaut, c'est **le relais de Vela**, et
vous pouvez remplacer le point de terminaison dans _Réglages → Avancé → Points de
terminaison des services_. Un point de terminaison s'applique à tous les réseaux
intégrés ; un réseau personnalisé conserve l'URL de relais que vous lui avez
donnée en l'ajoutant.

Une réserve honnête sur la compatibilité : l'application obtient les frais via une
méthode RPC propre à Vela (`vela_getInBandGasQuote`), et l'envoi échoue sans elle.
Le point de terminaison que vous visez doit donc faire tourner
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — l'instance de Vela ou
une que vous hébergez. Un bundler ERC-4337 générique comme **Pimlico** ou
**Alchemy** n'implémente pas cette méthode et ne fonctionnera donc pas de bout en
bout dans la version actuelle.

Celui qui opère le relais d'un réseau **encaisse les frais de ce réseau** — la
marge du relais sur chaque transaction et le dépôt d'activation du compte de gas.
Faites tourner votre propre vela-relay et ces frais financent votre infrastructure
plutôt que celle de Vela ; Vela ne prélève rien sur le trafic que vous envoyez
ailleurs.

<Callout type="warning" title="Le compte de gas fait partie du protocole vela-relay">
L'étape d'**activation du compte de gas** alimente un compte de relais dédié à
votre portefeuille sur chaque réseau. Si votre point de terminaison vise un
vela-relay auto-hébergé, le dépôt alimente le compte de votre propre relais, pas
celui de Vela.
</Callout>

### Activer le compte de gas (Vela Relay)

Sur le relais de Vela, votre première transaction sur chaque réseau **active un
compte de gas dédié**. L'application demande d'abord à la trésorerie du relais de
le financer pour vous — cela se fait silencieusement dans le flux d'envoi, et un
portefeuille sponsorisé ne voit jamais d'écran de financement. Ce n'est que si le
sponsoring est refusé que l'application affiche une demande de recharge : vous
envoyez un petit montant du token natif à l'adresse du compte de gas affichée, et
elle vous dit pourquoi le sponsoring n'était pas disponible.

**Vous payez vous-même les frais d'activation** dès qu'aucun sponsoring gratuit
n'est proposé — à savoir quand :

- **La trésorerie de Vela pour ce réseau est vide ou faible** — le fonds gratuit
  est temporairement épuisé sur cette chaîne.
- **Vous avez épuisé le quota gratuit** — le sponsoring est plafonné par
  portefeuille, au-delà des premières fois c'est à votre charge.
- **Le relais de Vela ne finance pas du tout ce réseau** — par exemple les
  **réseaux personnalisés ou de test que vous avez ajoutés**, pour lesquels Vela ne
  tient aucune trésorerie. (Dirigez-les vers votre propre relais si vous préférez
  éviter l'activation.)

Le dépôt d'activation est **non remboursable** — c'est le solde de départ du compte
de relais, il se recharge avec le temps grâce aux remboursements de gas, mais il
peut quand même s'épuiser et demander une **réactivation** plus tard. L'adresse du
relais peut aussi changer lors d'une mise à jour du service, ce qui exige une
nouvelle activation.

Les frais sortent de votre solde dans l'**actif de frais** que vous avez choisi —
le token natif par défaut. Si un envoi est bloqué pour cause de gas, c'est que
votre solde dans cet actif ne couvre pas les frais ; là où le relais propose du gas
en stablecoin, changer d'actif de frais sur l'écran de confirmation peut débloquer
la situation.

Quand vous envoyez le montant **maximum** d'un token natif, Vela met
automatiquement de côté de quoi payer le gas pour que la transaction n'échoue pas.

## Comment Vela parle à chaque réseau

Vela lit les soldes et soumet les transactions via un **pool de points de
terminaison RPC**, pas via un fournisseur unique. Il en rassemble depuis plusieurs
sources, les note selon la latence et la fiabilité, et **bascule automatiquement**
quand l'un est lent ou hors service — mettant temporairement les mauvais sur la
touche — de sorte qu'un seul nœud capricieux ne met jamais l'application hors
ligne.

Ensuite : [comment fonctionnent les passkeys](/fr/docs/passkeys).
