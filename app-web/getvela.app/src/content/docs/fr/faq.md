---
title: FAQ
description: Questions fréquentes sur Vela — conservation, passkeys, comptes intelligents, récupération, réseaux pris en charge, frais et confidentialité.
---

# FAQ

## Vela est-il auto-conservé ?

Oui. Votre portefeuille est un compte intelligent contrôlé par une clé que vous
seul pouvez utiliser, gardée par le système de votre appareil et jamais vue par
Vela. Vela ne peut ni déplacer, ni geler, ni récupérer vos fonds.

## Mon portefeuille est-il un compte normal ou un contrat ?

C'est un **compte intelligent Safe** (un contrat), opéré avec l'abstraction de
compte ERC-4337. C'est ce qui vous permet de signer avec une passkey, de lire
chaque transaction avant de l'approuver, et d'utiliser la même adresse sur chaque
réseau. L'architecture est dans le [livre blanc](/fr/docs/whitepaper).

## Il n'y a vraiment aucune phrase de récupération ?

Vraiment. Votre clé de signature est une passkey gardée par le système de votre
appareil, et Vela ne la voit jamais. Il n'y a pas douze mots à noter, à perdre ou
à se faire hameçonner. Pourquoi c'est sûr :
[comment fonctionnent les passkeys](/fr/docs/passkeys).

## Que se passe-t-il si je perds mon téléphone ?

Si votre passkey est synchronisée via le trousseau iCloud ou le gestionnaire de
mots de passe Google, vous vous connectez sur un nouvel appareil avec le même
compte et votre portefeuille revient. Le modèle complet et ses limites :
[récupération et connexion](/fr/docs/recovery).

## Quels réseaux et quels tokens sont pris en charge ?

Vela intègre **12 réseaux EVM** — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad et World Chain — plus
les réseaux personnalisés, avec les tokens natifs et les ERC-20. Votre adresse est
la même partout. Voir [réseaux et frais](/fr/docs/networks-and-fees).

## Combien ça coûte ?

Le portefeuille est gratuit et Vela n'a **aucun token**. Vous payez le **gas** du
réseau sur votre propre solde, plus des frais de relais. Le prix est donné par le
relais et affiché **avant que vous signiez**, décomposé en _frais réseau / frais de
relais / total_ — le coût exact de chaque transaction est sur l'écran de
confirmation, et le montant annoncé fait partie de ce que vous signez : il ne peut
donc pas changer ensuite. Les transactions très bon marché peuvent atteindre un
petit minimum. Sur Tempo, qui n'a pas de pièce native, le gas est réglé en
stablecoins USD. Chaque réseau demande aussi un petit **dépôt non remboursable
pour activer son compte relayeur de gas** (Vela peut le prendre en charge pour les
nouveaux utilisateurs) ; comme ce compte peut se vider, il faudra parfois le
réalimenter plus tard — ce n'est donc pas strictement unique. Détails dans
[réseaux et frais](/fr/docs/networks-and-fees).

## Que peut voir ou faire Vela (l'entreprise) ?

Vela stocke la clé **publique** de votre passkey et le **nom** que vous avez
choisi, pour permettre la connexion multi-appareils. Elle ne voit pas votre clé
privée, vos soldes sont lus depuis des chaînes publiques, et il n'y a aucune
inscription par e-mail. La version qui fait foi est la
[politique de confidentialité](/privacy).

## Vela est-il open source ?

Oui — le portefeuille et ses quatre services backend (données de chaîne, index de
passkeys, relais, taux de change) sont
[publics sur GitHub](https://github.com/mondaylabsltd/vela-wallet) sous licence
MIT, et vous pouvez les héberger vous-même.

## J'ai une question qui n'est pas ici.

Ouvrez une issue sur
[GitHub](https://github.com/mondaylabsltd/vela-wallet) ou écrivez-nous sur
[X](https://x.com/realvelawallet) ou [Telegram](https://t.me/velawallet).
