---
title: Guide d'auto-hébergement
description: "Tout ce que Vela fait tourner pour vous, le rôle de chaque élément, et comment le remplacer par le vôtre — le relais, l'index des clés publiques, les données de chaîne, les taux de change et les apps —, plus le seul élément que vous ne pouvez pas remplacer, et comment vous passer de getvela.app."
source: a093c30db3fb
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Guide d'auto-hébergement

Votre argent se trouve dans un contrat Safe on-chain, contrôlé par vos clés. Rien
de ce que Vela fait tourner ne peut le déplacer. Ce que Vela fait tourner, c'est la
machinerie qui rend le portefeuille pratique : un relais qui soumet vos
transactions, un index qui aide un nouvel appareil à retrouver votre portefeuille,
un annuaire de données de chaîne, un flux de taux de change, et les apps
elles-mêmes.

Cette page liste chacun de ces éléments, ce qui ne fonctionne plus sans lui, et
comment faire tourner le vôtre. Elle traite aussi du seul élément que vous ne
pouvez pas remplacer — le domaine auquel appartiennent vos passkeys — et de ce
qu'il faut faire si getvela.app disparaît.

<Callout type="info" title="À qui s'adresse cette page">
Vous devez être à l'aise avec un terminal, avec Docker ou Cloudflare Workers, et
savoir approvisionner une adresse sur une chaîne. Rien de tout cela n'est
nécessaire pour utiliser Vela au quotidien.
</Callout>

## Vue d'ensemble

| Élément | Ce qu'il fait | Par défaut chez Vela | Remplaçable ? | Sans lui |
| --- | --- | --- | --- | --- |
| **Relais** | Reçoit votre opération signée, avance le gas, la soumet, encaisse les frais que vous avez signés | `vela-relay-cf.getvela.app` | Oui — faites tourner [vela-relay](#relay) et faites pointer le portefeuille vers lui | Vous ne pouvez pas envoyer |
| **Index des clés publiques** | Enregistre on-chain les clés d'un nouveau portefeuille ; répond à « de quel portefeuille cette clé fait-elle partie ? » | `p256-index-v2.getvela.app` | Oui — faites tourner [p256-index](#index) | Impossible de créer de nouveaux portefeuilles ; la connexion se rabat sur la lecture de la chaîne |
| **Contrat de registre** | L'enregistrement public et permanent des clés de chaque portefeuille | `0x94fD…1EA9` sur Gnosis | Inutile — il n'appartient à personne ; le portefeuille le lit directement | — |
| **Données de chaîne** | Informations sur les réseaux, listes de jetons, logos, descripteurs de signature lisible | `ethereum-data.getvela.app` | Oui — faites tourner [ethereum-data](#chain-data) | Pas de listes de jetons ni de logos ; moins de transactions décodées ; l'ajout de réseaux échoue |
| **Taux de change** | Valeurs en monnaie fiduciaire dans votre devise d'affichage | `vela-currency.getvela.app` | Oui — faites tourner [vela-currency](#exchange-rates) ou toute source compatible Frankfurter | Les apps se rabattent sur les taux Chainlink on-chain quand elles le peuvent (l'app de bureau affiche des USD) |
| **Nœuds RPC** | Lecture des soldes, simulation des transactions | Points d'accès publics par réseau | Oui — réseau par réseau, dans Réglages → Réseaux | Vela bascule d'un point d'accès à l'autre |
| **Les apps** | Le portefeuille lui-même | wallet.getvela.app, versions publiées | Oui — [compilez-les](#web-app) | — |
| **getvela.app** | Le domaine auquel appartiennent vos passkeys | — | **Non** — voir [plus bas](#if-getvela-app-disappears) | — |

Quelques services tiers, qui n'appartiennent pas à Vela, sont également
sollicités : les bases publiques de sélecteurs de fonctions (sourcify, openchain,
4byte), utilisées en dernier recours pour décoder une transaction, l'annuaire
d'authentificateurs qui donne le nom du modèle de votre clé de sécurité, et les
serveurs de tunnel d'Apple et de Google quand vous signez avec un téléphone en
scannant un QR code.

## Le seul élément irremplaçable : le domaine des passkeys

<span id="if-getvela-app-disappears"></span>

Une passkey appartient au site web pour lequel elle a été créée. Les clés de Vela
sont créées pour `getvela.app`. Les navigateurs ne les proposent qu'aux pages de
getvela.app ou de ses sous-domaines (ou aux origines que getvela.app déclare comme
associées), et les passkeys intégrées d'un téléphone ne fonctionnent que dans les
apps dont getvela.app se porte garant. Hors du navigateur, la règle est plus
souple : Chrome permet à une extension autorisée pour getvela.app de les utiliser,
et un programme sur votre ordinateur peut demander directement une signature
getvela.app à une clé de sécurité ou à un téléphone — c'est ainsi que fonctionnent
les apps que vous compilez vous-même, et c'est pourquoi le logiciel que vous faites
tourner compte. Deux conséquences.

**Une copie du portefeuille web sur votre propre domaine est un autre
portefeuille.** Servi depuis `wallet.example.com`, le même code crée des passkeys
pour `wallet.example.com` — de nouvelles clés, donc une nouvelle adresse. Il ne
peut pas signer pour un portefeuille créé sur wallet.getvela.app. Cette copie reste
utile : pour un portefeuille que vous y créez, ou pour faire tourner toute la pile
vous-même, à partir de zéro.

**Pour un portefeuille existant, ces moyens fonctionnent encore si getvela.app est
hors ligne ou a disparu :**

| Moyen d'accès | Clés utilisables | Où l'obtenir |
| --- | --- | --- |
| L'**extension Vela pour navigateur** (navigateurs Chromium : Chrome, Edge, Brave) | Toute clé que le navigateur peut atteindre : la passkey de cet appareil, une clé de sécurité USB (NFC quand l'ordinateur le permet), un téléphone via QR code | Une archive zip publiée sur [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases), ou [compilez-la](#web-app) |
| Une **app de bureau ou mobile que vous compilez vous-même** | Un téléphone via QR code, et les clés de sécurité USB | [Compilez-la](#web-app) |
| Les **apps des stores et les apps de bureau notariées** | Un téléphone via QR code et les clés de sécurité, toujours ; les passkeys « cet appareil », seulement tant que le système d'exploitation peut encore vérifier l'app auprès de getvela.app | Versions GitHub (stores plus tard) |

L'extension peut utiliser les clés `getvela.app` parce que Chrome permet à une
extension autorisée pour un site d'utiliser les passkeys de ce site. Le navigateur
vérifie cette autorisation localement ; nous avons constaté que cela fonctionne,
mais pas encore avec le domaine réellement hors ligne. Une app que vous compilez
vous-même peut utiliser un téléphone ou une clé de sécurité parce que Vela
communique directement avec eux ; la passkey propre au téléphone (« cet appareil »)
exige que l'app soit signée par Vela, ce qui n'est pas le cas de la vôtre.

La [page de signature](/fr/docs/clear-signing-self-host) n'est pas, à elle seule,
un moyen d'accès : elle signe les demandes qu'un autre programme lui envoie, et
aucune app Vela ne lui en envoie encore.

<Callout type="warning" title="Qui contrôle le domaine peut demander une signature">
Toute page servie depuis getvela.app ou l'un de ses sous-domaines — ou par
quiconque contrôlera le domaine à l'avenir — peut demander une signature à vos
clés, et l'invite du système affiche « getvela.app », pas la transaction. C'est
ainsi que fonctionnent les passkeys partout. C'est pour cette raison que le site
de Vela interdit à ses propres pages d'utiliser les passkeys. C'est aussi pourquoi
l'extension et les apps compilées vous-même comptent : elles embarquent leur propre
code, même si, par défaut, elles récupèrent encore les descripteurs et utilisent
des services hébergés sous getvela.app.
</Callout>

## Faire pointer le portefeuille vers vos services

Chaque app propose quatre champs dans **Réglages → Avancé → Points d'accès des
services** (dans l'app de bureau, **Réglages → Points d'accès des services**) : données
de chaîne, index des passkeys, relais Vela et taux fiat — affichés dans l'app sous
les noms **Index des données de chaîne**, **Index des clés d'accès**,
**Vela Relay** et **Taux fiat**. Chaque champ affiche la valeur par défaut de Vela
tant que vous ne la changez pas ; **Réinitialiser par défaut** rétablit les quatre.
Pour le relais, l'index et les données de chaîne, le portefeuille appelle
`/api/health` et affiche un badge, vert seulement si le point d'accès annonce le
bon service et renvoie `status: "ok"`. Ce que vous saisissez est enregistré dans
tous les cas — attendez le vert.

| Service | `service` dans `/api/health` |
| --- | --- |
| Relais | `vela-relay` |
| Index des clés publiques | `webauthn-p256-publickey-registry` |
| Données de chaîne | `ethereum-data` |
| Taux de change | pas vérifié par son nom — doit renvoyer une liste de taux en base USD |

Dans quelle mesure chaque app respecte ces réglages aujourd'hui :

| App | Points d'accès des services | RPC par réseau |
| --- | --- | --- |
| Web et extension | Données de chaîne, relais et taux fiat. L'index des passkeys sert à rechercher les noms, mais la création d'un portefeuille et la connexion utilisent encore l'index de Vela | Oui |
| Bureau | Les quatre ; un nouvel index des passkeys prend effet après un redémarrage ou une déconnexion | Oui |
| Android | Les quatre, sauf la recherche de noms pour les adresses, qui interroge encore l'index de Vela | Oui |
| iOS | **Pas encore** : la page affiche des valeurs fictives et n'enregistre rien. L'index des passkeys peut être modifié sur l'écran de connexion quand celui par défaut est injoignable | Lecture seule |

Ces lacunes sont des bugs, et elles sont suivies.

## Faire tourner votre propre relais

<span id="relay"></span>

Le relais est [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust,
MIT). Un seul déploiement sert toutes les chaînes : le portefeuille appelle
`https://your-relay/<chainId>`. Ce doit être vela-relay — le portefeuille demande
un devis de frais avec une méthode propre à Vela que les bundlers ERC-4337
génériques n'implémentent pas.

**Ce qu'il vous faut**

- Soit Docker, plus un serveur Redis et un serveur [Iggy](https://iggy.apache.org)
  que vous faites déjà tourner, soit un compte Cloudflare avec l'offre **Workers
  Paid**, ainsi que Node.js et une chaîne d'outils Rust (avec la cible
  `wasm32-unknown-unknown`) sur votre machine.
- Un `OPERATOR_SECRET` (hexadécimal, 32 octets au moins). Il dérive une adresse de
  trésorerie et un ensemble d'adresses de relayeurs, identiques sur toutes les
  chaînes. Gardez-le secret : il contrôle les fonds du relais.
- Du gas sur chaque chaîne que vous voulez servir : envoyez la monnaie de la chaîne
  (du pathUSD sur Tempo) à votre adresse de trésorerie. La trésorerie
  réapprovisionne les relayeurs.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# dans .env : VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL si vous faites tourner vos propres données de chaîne,
# et VELA_RELAY_IMAGE pointant vers une image publiée de confiance (voir docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Préférez l'image publiée : compiler depuis les sources avec
`docker compose up --build` peut échouer avec le Dockerfile actuel. Sans Docker,
`cargo run --release --bin vela-relay` le lance directement.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# vos propres données de chaîne : ajoutez "VELA_RELAY_CHAIN_DIRECTORY_URL" sous "vars" dans wrangler.jsonc
npx wrangler deploy
```

**Vérifier**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # votre adresse de trésorerie sur Gnosis, et si elle a besoin de gas
```

Saisissez ensuite `https://your-relay` dans le champ **Vela Relay**.

**À savoir**

- Les frais payés par le portefeuille vont à votre trésorerie. Le portefeuille les
  calcule de la même façon quel que soit le relais utilisé (voir
  [réseaux et frais](/fr/docs/networks-and-fees)).
- Un réseau personnalisé ajouté avant le changement de relais garde l'adresse de
  relais avec laquelle il a été ajouté.
- Le relais lit les informations de chaque chaîne et les stablecoins qu'il accepte
  dans un annuaire de chaînes : `ethereum-data.getvela.app`, sauf si vous faites
  pointer `VELA_RELAY_CHAIN_DIRECTORY_URL` vers [le vôtre](#chain-data). Cette variable existe à partir de vela-relay v0.9.6 ; les versions antérieures lisent toujours la copie de Vela.

## Faire tourner votre propre index des clés publiques

<span id="index"></span>

L'index est [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust, MIT).
Quand un portefeuille est créé, il vérifie la preuve de chaque clé, puis inscrit le
groupe dans le **contrat de registre** sur Gnosis et en paie le gas. Continuez
d'utiliser le registre existant à `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` :
il n'a pas de propriétaire, n'importe quelle adresse approvisionnée peut y écrire,
et toutes les apps Vela le lisent directement. Un registre à vous leur serait
invisible.

**Ce qu'il vous faut**

- Docker avec Redis et Iggy (la version serveur), ou un compte Cloudflare (la
  version Worker, dont le README précise que l'écriture on-chain n'a pas encore été
  testée de bout en bout).
- Une clé privée Gnosis avec des xDAI. Enregistrer un portefeuille coûte environ
  1,1 million de gas avec une clé, et environ 3,6 millions avec sept.
- Ces réglages :

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

`P256_INDEX_DOMAIN_REGISTRY` est indispensable, même si le fichier d'exemple du
serveur l'omet : sans lui, le serveur émet des défis que le contrat rejette, et
tous les enregistrements échouent.

**Lancer et vérifier**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

Le serveur écoute en HTTP simple (port 11256 par défaut) ; placez un proxy TLS
devant lui, car le portefeuille n'accepte que des points d'accès `https://`. À
l'heure où ces lignes sont écrites, le Dockerfile des sources peut ne pas
compiler ; la compilation avec Cargo, elle, fonctionne.

**Si aucun index ne répond**, les portefeuilles existants fonctionnent toujours : à
la connexion, l'app lit le contrat de registre sur Gnosis (puis sur Ethereum) via
vos nœuds RPC. Un portefeuille à clé unique peut même être reconstruit à partir de
deux signatures, sans aucun registre. Créer un nouveau portefeuille exige en
revanche un index, car il faut bien que quelqu'un paie l'enregistrement.

## Faire tourner vos propres données de chaîne

<span id="chain-data"></span>

Les données de chaîne, c'est [ethereum-data](https://github.com/atshelchin/ethereum-data)
(MIT) : du JSON statique et des images pour environ 2 600 réseaux et leurs jetons,
plus les descripteurs ERC-7730 que Vela utilise pour expliquer les transactions.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

Son README explique aussi comment compiler depuis les sources et déployer sur
Cloudflare. Servez-le en HTTPS et saisissez l'adresse dans le champ **Index des
données de chaîne**.

Le relais lit aussi ces fichiers, y compris un champ propre à Vela (la liste
`stables` détermine quels stablecoins peuvent payer les frais). Faites-le pointer
vers votre copie avec `VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data` ; il
met en cache l'entrée de chaque réseau pendant une heure.

## Faire tourner vos propres taux de change

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) republie
les taux quotidiens de la Banque centrale européenne. Il ne demande aucune clé.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Saisissez `https://your-host/v2/rates?base=USD` dans le champ **Taux fiat**. Tout
service compatible Frankfurter fonctionne aussi. Gardez `?base=USD` : toutes les
conversions partent de cette base.

## Compiler les apps vous-même

<span id="web-app"></span>

Toutes les apps sont dans [un seul dépôt](https://github.com/mondaylabsltd/vela-wallet)
(MIT). Le README détaille les étapes de compilation de chaque app ; en résumé :

| App | Compilation | Signe pour votre portefeuille getvela.app existant ? |
| --- | --- | --- |
| Extension de navigateur | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, puis chargez `extension/dist` comme extension non empaquetée dans `chrome://extensions` | Oui, avec n'importe quelle clé |
| Portefeuille web | `cd app-web/vela-wallet && pnpm install && pnpm build` ; se déploie comme Cloudflare Worker | Non — sur votre domaine, c'est un autre portefeuille (voir plus haut) |
| Bureau | `cd app-desktop/vela-wallet && cargo run` (scripts de packaging dans son README) | Oui, avec un téléphone via QR code ou une clé de sécurité USB |
| Android | Générez les bindings du cœur, puis `./gradlew :app:installDebug` | Oui, avec un téléphone via QR code ou une clé de sécurité USB |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, puis compilez dans Xcode avec votre propre équipe | Oui, avec un téléphone via QR code ou une YubiKey USB-C / Lightning (firmware 5.8 ou plus récent) |

La passkey « cet appareil » d'une app compilée vous-même ne fonctionnera pas pour
les portefeuilles getvela.app : Apple et Google ne laissent que les apps signées
par Vela utiliser les passkeys `getvela.app`.

## Ajouter un réseau que Vela n'intègre pas

Vela fonctionne sur toute chaîne EVM qui dispose du précompilé P-256 et des
contrats standards qu'il vérifie. La page [Configurer une chaîne](/fr/chain-setup)
vous dit ce qui manque à une chaîne et déploie ce que tout le monde peut déployer ;
[réseaux et frais](/fr/docs/networks-and-fees) explique les conditions requises.
Une lacune : un portefeuille à plusieurs clés a aussi besoin de la fabrique de
signataires passkey de Safe sur cette chaîne, ce que la vérification ne contrôle
pas encore — sans elle, seule la première clé peut signer sur cette chaîne.

## Ce qui pointe encore vers Vela après tout cela

Si vous remplacez tout ce qui précède, il reste :

- **L'annuaire d'authentificateurs** qui donne le nom des modèles de clés de
  sécurité — purement cosmétique ; à défaut, les apps affichent un nom générique.
- **Les fichiers d'association de getvela.app**, dont les apps des stores ont
  besoin pour les passkeys « cet appareil ». Un téléphone ou une clé de sécurité
  n'en a pas besoin.

Et ces éléments-là n'appartiennent pas à Vela : les bases publiques de sélecteurs,
les serveurs de tunnel d'Apple et de Google pour la connexion par téléphone, et les
fournisseurs RPC que vous choisissez.

Ensuite : [la page de signature que vous pouvez faire tourner vous-même](/fr/docs/clear-signing-self-host).
