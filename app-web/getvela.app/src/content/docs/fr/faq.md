---
title: FAQ
description: "Des réponses courtes sur la conservation des fonds, les clés, la récupération, les réseaux, les frais, ce que Vela peut voir, l'open source, et ce qui se passe si Vela disparaît."
source: 7446e22f990d
---

# FAQ

## Vela est-il un portefeuille en auto-conservation ?

Oui. Votre portefeuille est un compte intelligent Safe contrôlé uniquement par vos
clés, qui restent dans vos appareils, votre gestionnaire de mots de passe ou vos
clés de sécurité. Vela ne détient aucune clé ni aucun rôle sur ce compte ; il ne
peut donc ni déplacer, ni geler, ni récupérer vos fonds de lui-même. C'est en
revanche lui qui écrit le logiciel qui demande à vos clés de signer — voir le
[modèle de menaces](/fr/docs/whitepaper).

## Il n'y a vraiment aucune phrase de récupération ?

Vraiment. Vos clés sont des passkeys, et une passkey n'a aucun secret que l'on
puisse noter ou taper. Voir [comment fonctionnent les passkeys](/fr/docs/passkeys).

## De quoi ai-je besoin pour créer un portefeuille ?

Un appareil compatible avec les passkeys (un téléphone ou un ordinateur récent avec
Face ID, une empreinte ou Windows Hello), ou deux clés de sécurité matérielles. Ni
e-mail, ni compte, ni solde de départ. Vous pouvez créer le portefeuille avec
jusqu'à sept clés ; elles ne peuvent pas être ajoutées plus tard. Voir
[créer votre portefeuille](/fr/docs/create-wallet).

## Que se passe-t-il si je perds mon téléphone ?

Connectez-vous sur un nouvel appareil avec n'importe quelle autre clé : la même
passkey synchronisée via le trousseau iCloud ou le gestionnaire de mots de passe de
Google, un autre téléphone, ou votre clé de sécurité. Si le téléphone détenait
votre seule clé et que cette clé n'était pas synchronisée, le portefeuille ne peut
pas être récupéré. Voir [récupération et connexion](/fr/docs/recovery).

## Quels réseaux et quels jetons sont pris en charge ?

24 réseaux EVM intégrés, dont Ethereum, Base, Arbitrum, Optimism, Polygon, BNB
Chain, Gnosis et Avalanche, plus tout réseau EVM que vous ajoutez et qui remplit
les conditions requises. Les monnaies natives et les jetons ERC-20. L'adresse est
la même sur tous les réseaux. Voir [réseaux et frais](/fr/docs/networks-and-fees).

## Combien ça coûte ?

- **Les apps :** le portefeuille web, l'extension de navigateur et les apps de
  bureau sont gratuits. Les apps iOS et Android seront proposées en achat unique
  sur les stores ; vous pouvez aussi compiler n'importe quelle app gratuitement
  depuis les sources.
- **Chaque transaction :** des frais payés depuis votre portefeuille au relais qui
  la soumet — celui de Vela, sauf si vous faites pointer le portefeuille vers un
  autre relais ou faites tourner le vôtre. Ils couvrent le gas plus la marge du
  relais, avec un minimum d'environ 0,01 $. Le montant exact figure sur l'écran de
  confirmation avant que vous signiez, et fait partie de ce que vous signez. Il n'y
  a ni dépôt ni abonnement.
  [Comment les frais sont calculés](/fr/docs/networks-and-fees#fee).
- **Aucun jeton.** Vela n'en a pas et n'en prévoit pas.

## Puis-je utiliser Vela avec des dApps ?

Oui, via l'extension Vela pour navigateur (Chrome, Edge, Brave) et le navigateur
intégré aux apps bureau (macOS, Windows), iOS et Android. Le portefeuille web sur
wallet.getvela.app ne se connecte pas aux dApps. Voir [installer Vela](/fr/docs/install#dapps).

## Que peut voir ou faire Vela ?

Vela ne peut pas lire vos clés ni déplacer vos fonds de lui-même. Ses services
voient votre adresse IP et ce que l'app leur demande : l'index voit vos clés
publiques et le nom de votre portefeuille quand il enregistre un nouveau
portefeuille, ainsi que les adresses que vous recherchez ; le relais voit votre
adresse, les opérations que vous soumettez et le point d'accès RPC utilisé par votre
app ; le service de données de chaîne voit sur quels jetons et quels contrats votre
app l'interroge. Ce qui devient public on-chain est listé sur
[créer votre portefeuille](/fr/docs/create-wallet#what-is-public). La
[politique de confidentialité](/privacy) en est la version complète, qui fait foi.

## Vela est-il open source ?

Oui, entièrement, sous licence MIT : les apps du portefeuille et le cœur, le relais,
l'index des clés publiques, le service de taux de change et l'annuaire de données
de chaîne, sur [GitHub](https://github.com/orgs/mondaylabsltd/repositories). Vous
pouvez faire tourner chaque service vous-même — voir le
[guide d'auto-hébergement](/fr/docs/self-hosting).

## Vela est-il audité ?

Les contrats qui détiennent votre argent — Safe et ses modules, et l'EntryPoint
ERC-4337 — sont audités. Le code de Vela, lui, ne l'est pas, et aucun audit n'est
programmé. Voir [audits et problèmes connus](/fr/docs/security-audits).

## Et si Vela ferme ?

Vos fonds restent dans votre Safe, on-chain. Pour un portefeuille existant,
l'extension Vela pour navigateur et les apps que vous compilez vous-même continuent
de fonctionner sans getvela.app, et chaque service est open source, donc
exploitable par quelqu'un d'autre. Le
[guide d'auto-hébergement](/fr/docs/self-hosting#if-getvela-app-disappears) liste
ces solutions et leurs limites.

## J'ai une question qui n'est pas ici.

Ouvrez une issue sur [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues),
ou contactez-nous sur [X](https://x.com/realvelawallet) ou
[Telegram](https://t.me/velawallet).
