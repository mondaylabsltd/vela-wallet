---
title: Introduction
description: Ce qu'est Vela, à qui il s'adresse, et les idées derrière un portefeuille intelligent auto-conservé sans phrase de récupération.
---

# Introduction

Vela est un **portefeuille intelligent auto-conservé** pour les réseaux EVM. Les
clés sont à vous, mais il n'y a aucune phrase de récupération à noter — vous
signez avec une passkey, par le visage ou l'empreinte.

Cette documentation explique comment démarrer, créer un portefeuille, déplacer des
tokens, et comprendre le modèle de sécurité qui tient tout cela.

## La version courte

- **Auto-conservé.** Vos fonds sont contrôlés par une clé que vous seul pouvez
  utiliser. Vela (l'entreprise) ne peut ni déplacer, ni geler, ni récupérer votre
  argent.
- **Aucune phrase de récupération.** Votre clé de signature est une passkey
  gardée par le matériel sécurisé de votre appareil. Il n'y a pas douze mots à
  perdre ou à se faire hameçonner.
- **Un compte intelligent Safe.** Chaque portefeuille est un contrat
  [Safe](https://github.com/safe-fndn/safe-smart-account), opéré via l'abstraction
  de compte ERC-4337 — c'est précisément ce qui vous permet de signer avec une
  passkey et de lire chaque transaction avant de l'approuver.
- **12 réseaux, une seule adresse.** Ethereum, BNB Chain, Polygon, Arbitrum,
  Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad et World Chain — plus
  les réseaux que vous ajoutez — tous à la même adresse.
- **Une signature lisible.** Là où un descripteur existe, la transaction est
  traduite en intention lisible (ERC-7730) ; sinon, Vela retombe sur un décodage
  au mieux et vous avertit. Les appels qu'il ne sait pas lire sont signalés, pas
  cachés.
- **Open source.** Le portefeuille et tous ses services sont
  [publics sur GitHub](https://github.com/mondaylabsltd/vela-wallet), pour que
  chacun puisse vérifier ce qu'ils font.
- **Logiciel en alpha.** Vela fonctionne et contient de l'argent réel, mais il n'a
  pas des années de production derrière lui. Commencez petit. Le
  [billet sur l'alpha](/blog/vela-is-in-alpha) explique ce que cela implique.

## À qui il s'adresse

Vela est fait pour les gens qui veulent une vraie auto-conservation sans le piège
de la gestion d'une phrase de récupération — et pour ceux qui s'y sont déjà
brûlés. Si vous savez déverrouiller votre téléphone, vous savez utiliser Vela.

## Où aller ensuite

- [Installer Vela](/fr/docs/install) — ça tourne dans votre navigateur, rien à
  télécharger.
- [Créer votre portefeuille](/fr/docs/create-wallet) — votre premier portefeuille
  en une minute environ.
- [Comment fonctionnent les passkeys](/fr/docs/passkeys) — le modèle de sécurité,
  expliqué simplement.
- [Livre blanc](/fr/docs/whitepaper) — l'architecture complète et le modèle de
  confiance.

Si le *pourquoi* vous intéresse plus que le *comment*, le [blog](/blog) raconte
comment Vela se construit.
