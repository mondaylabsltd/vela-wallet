---
title: Trusted Signer
description: "Une page d'un seul fichier, sur sign.getvela.app, qui décode une demande et la signe elle-même avec votre passkey — ce qu'elle vérifie, quelles apps l'utilisent, et comment la reconstruire ou faire tourner votre propre copie."
source: 43a1af6ffdcd
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela décode chaque transaction avant que vous l'approuviez, et ce décodage est fait
honnêtement — mais il est fait par la même app que celle qui a construit la
transaction. Si l'app, ou la façon dont elle vous parvient, est altérée, elle peut
vous montrer une chose et en signer une autre. C'est exactement ce qui est arrivé à
[Bybit](/fr/docs/bybit-attack).

Le Trusted Signer existe pour séparer les deux : l'app ne fait que transmettre la
demande, et la vérification comme la signature ont lieu sur une page à part — une
page que vous pouvez lire de bout en bout, reconstruire octet pour octet, ou faire
tourner vous-même.

## Où il tourne

La page officielle est servie depuis **sign.getvela.app**. Les apps de bureau
(macOS, Windows, Linux), iPhone et Android peuvent lui envoyer une demande : l'app
ouvre la page dans un onglet du navigateur avec la demande dans le lien, vous la
vérifiez et la signez là-bas avec votre passkey, et la page renvoie la signature à
l'app par un lien `velawallet://`. Le portefeuille web ne peut pas l'utiliser.

Il est facultatif. Vous le choisissez comme mode de signature quand vous créez un
portefeuille ou vous connectez, et à partir de là chaque signature de ce portefeuille
sur cet appareil passe par lui. Il peut aussi créer les clés du portefeuille. Sur
`sign.getvela.app`, il utilise les mêmes passkeys `getvela.app` que les apps.

<Callout type="info" title="Testé jusqu'ici">
Parcours de bout en bout enregistrés sur la page publiée : Android, et Windows 11
(création d'un portefeuille et connexion). Les apps macOS, Linux et iPhone utilisent
le même câblage ; aucune n'a encore de parcours complet enregistré.
</Callout>

## Ce qu'il fait avant de signer

- **Il décode lui-même la demande.** Ce que fait l'appel, pour qui et pour quel
  montant, à partir de la calldata — y compris les appels imbriqués dans un lot.
- **Il ne signe qu'un condensat qu'il a calculé.** Les condensats EIP-191, EIP-712,
  SafeOp et SafeMessage sont calculés dans la page, jamais repris du demandeur ; des
  tests confrontent les condensats SafeOp et SafeMessage à `vela-core`, le code
  qu'utilise le portefeuille, et l'app refuse une signature portant sur tout autre
  condensat que celui qu'elle a calculé elle-même.
- **Il vérifie que la transaction est bien celle qui a été demandée.** L'appel
  demandé par le site doit réellement se trouver dans l'opération signée, sinon la
  page refuse.
- **Il signale quand une approbation est illimitée.** Il ne peut pas modifier un
  montant — il signe les octets reçus ou rien —, donc une approbation ou un permit
  illimité (2^128 ou plus sur cette page) s'affiche en rouge avec cette raison et
  peut être signé tel quel ; un plafond on-chain se choisit sur l'écran d'approbation
  du portefeuille lui-même, avant que la demande n'arrive ici. Une approbation
  portant sur toute une collection de NFT est refusée.
- **Il refuse ce qu'il ne peut pas garantir :** `eth_sign`, une méthode qu'il ne
  connaît pas, un jeton envoyé à son propre contrat, une opération qu'il ne sait pas
  lire, et une connexion dont le défi (challenge) a été fourni par le demandeur.
- **Il refuse ce qui livrerait votre compte,** selon la règle qu'appliquent les
  apps : un appel de votre compte à l'une de ses propres fonctions de propriétaires,
  de modules, de guard ou de fallback, y compris dans un lot ; un `delegatecall`,
  sauf vers le contrat MultiSend de Safe, qui regroupe les appels d'une opération ;
  et une signature `SafeTx`. Il vérifie chaque appel de l'opération assemblée par
  l'app, pas seulement ceux que le site a demandés.
- **Il affiche l'adresse du compte et un identicon calculé dans la page.**
  Destinataires et contrats ne sont jamais nommés à partir de la demande — seule la
  table relue de la page elle-même peut nommer un contrat. Le nom du compte, que l'app
  envoie pour que vous puissiez choisir la bonne passkey, s'affiche à côté de son
  adresse.
- **Il demande la vérification de l'utilisateur** (votre empreinte, votre visage ou
  votre code PIN) à chaque signature.

## Ce qu'il n'a pas, volontairement

- **Aucun éditeur.** La demande est figée à son arrivée : vous la signez ou non. Un
  sélecteur de frais ou un éditeur d'allocation réécrirait la calldata — c'est
  précisément le mal que cette page existe pour empêcher.
- **Aucun accès au réseau.** La page est un fichier unique dont la politique de
  sécurité du contenu (`default-src 'none'`) est inscrite dans ses propres octets :
  elle ne peut donc rien récupérer, ni ouvrir de connexion, ni charger d'image. La
  seule chose qui en sort est sa réponse, quand elle suit le lien de rappel contenu
  dans la demande (`velawallet://` quand c'est une app Vela qui a demandé). Les logos
  de jetons sont dessinés sous forme de lettres.

## Ce que l'app vérifie en retour

L'app ne fait pas confiance à la page non plus. Elle n'accepte une signature que si
le défi signé est le condensat **que l'app a calculé**, que la vérification de
l'utilisateur a eu lieu, que la clé est l'une de celles de votre portefeuille, et que
la signature P-256 est valide pour cette clé.

## Chaque version publiée, vérifiable

Chaque version est construite à partir de `app-web/trusted-signer/src/` en un seul
fichier, de façon reproductible — Bun et Node produisent les mêmes octets — et
publiée à sa propre adresse, `sign.getvela.app/b/<sha256>/sign.html`, à côté de
toutes les versions précédentes. La liste se trouve à `sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

Au démarrage, l'app de bureau télécharge la version publiée qu'elle va ouvrir, en
calcule l'empreinte et la compare aux versions qu'elle embarque. Le verdict est
seulement journalisé, et une page qui ne correspond pas est ouverte quand même. Les
apps mobiles ne font pas encore cette vérification.

## Faire tourner votre propre copie

Les Réglages conservent l'adresse de la page qu'ouvrent vos apps : vous pouvez donc y
mettre celle de votre propre déploiement — n'importe quelle adresse HTTPS, ou
`localhost` pour tester. Construisez la page avec `bun samples/build-single.mjs` (ou
`node`) et copiez `dist/` sur votre hébergement.

Une copie sur votre propre domaine signe avec des passkeys créées pour **ce**
domaine, et non avec les passkeys `getvela.app` — c'est donc un moyen de créer et
d'utiliser un portefeuille dont les clés relèvent de votre domaine, pas un moyen de
signer pour un portefeuille `getvela.app` existant. Toutes les clés d'un portefeuille
partagent un même domaine.

Le code, ainsi que les scripts qui le construisent et le vérifient, se trouvent dans
`app-web/trusted-signer/`.
