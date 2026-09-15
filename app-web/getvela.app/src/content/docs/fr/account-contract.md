---
title: Le contrat de compte
description: Votre portefeuille Vela est un Safe v1.4.1 non modifié. Rien dans le chemin des contrats n'a été écrit par nous — voici ce que cela vous apporte, et ce que cela coûte.
---

# Le contrat de compte

Votre portefeuille n'est pas une structure de données privée d'une application.
C'est un compte intelligent **Safe v1.4.1** — le même contrat qui garde des
trésoreries bien plus grandes que tout ce que Vela verra jamais — déployé
exactement tel que Safe le publie, sans aucune modification.

La phrase est courte, les conséquences ne le sont pas : cette page les détaille.

## Rien dans ce chemin n'est à nous

Quatre contrats se tiennent entre vous et votre argent. Vela n'en a écrit aucun :

| Contrat | Qui l'a écrit |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (le compte lui-même, un proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (vérifie votre clé P-256) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | Les auteurs d'ERC-4337 |

Il n'existe pas de contrat Vela. Le dépôt ne contient aucun Solidity — vérifiable
en une commande :

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # n'affiche rien
```

Quand Vela ajoute un réseau, il déploie **ces** contrats-là, à leurs adresses
canoniques. Il ne déploie aucun contrat de sa conception et ne détient aucun rôle
privilégié sur le vôtre : pas de clé d'administration, pas de chemin de mise à
niveau, pas de module que nous pourrions ajouter.

## Pourquoi « non modifié » est le mot qui compte

Beaucoup de portefeuilles se disent bâtis sur « un Safe », « un fork de Safe » ou
« un compte inspiré de Safe ». Un fork, c'est un nouveau contrat avec une vieille
réputation. Concrètement, les différences :

**Les audits portent sur ce que vous utilisez réellement.** Les rapports d'audit
de Safe couvrent le bytecode de ces versions précises. Les audits d'un fork
couvrent le code d'avant le fork. Si un portefeuille a modifié le contrat de
compte, chaque audit qu'il cite est l'audit d'autre chose — et la modification est
justement la partie que personne n'a regardée.

**L'écosystème traite votre compte comme un Safe, parce que c'en est un.** Les
explorateurs de blocs le décodent. L'outillage de transactions de Safe le
comprend. Si Vela disparaît demain, votre portefeuille n'est pas un format
orphelin — c'est le compte intelligent le mieux outillé d'Ethereum, et n'importe
quelle interface compatible Safe peut le piloter. C'est ce qui fait de
[« si Vela disparaît, votre portefeuille reste »](/fr/docs/why-vela) une phrase
sur des contrats plutôt que sur nos intentions.

**La surface d'attaque est celle que tout le monde surveille aussi.** Un contrat
de compte sur mesure n'est surveillé que par son auteur. Celui-ci est surveillé
par tous ceux qui gardent de l'argent dans un Safe.

## Ce que cela coûte

Être standard n'est pas gratuit, et les compromis sont réels :

- **Le gas.** Un compte intelligent vérifie une signature on-chain. Comptez environ
  1,5 à 3 fois le gas d'un simple transfert EOA, selon la chaîne. Voir
  [réseaux et frais](/fr/docs/networks-and-fees).
- **Le compte doit être déployé.** Votre adresse est calculée avec `CREATE2` avant
  que rien n'existe on-chain, vous pouvez donc recevoir tout de suite, mais la
  première transaction sortante paie le déploiement du contrat.
- **Toutes les chaînes ne sont pas éligibles.** Le signataire WebAuthn vérifie une
  signature P-256 on-chain, ce qui exige le précompilé **RIP-7212**. Vela refuse
  d'activer un réseau qui ne l'a pas plutôt que de retomber sur un vérificateur
  plus faible.
- **Le risque de Safe est désormais le vôtre.** Faire confiance à un contrat très
  utilisé reste faire confiance à un contrat. Ce que Vela peut dire, c'est qu'il
  n'a pas ajouté par-dessus une deuxième chose à qui faire confiance.

## Ce qui est audité et ce qui ne l'est pas

Les contrats de Safe et le module signataire WebAuthn sont audités par des tiers,
et ces rapports sont publics. **Le code applicatif de Vela n'a pas fait l'objet
d'un audit indépendant**, et aucun n'est programmé — c'est un objectif pour le
moment où le projet pourra le financer, pas un engagement daté. Chaque contrat
dont Vela dépend, son rapport d'audit et les problèmes que nous suivons sont
listés dans [audits et problèmes connus](/fr/docs/security-audits).

## Vérifiez par vous-même

Votre compte est on-chain. Ouvrez-le dans un explorateur de blocs et lisez
l'adresse d'implémentation : ce sera le déploiement canonique de Safe en v1.4.1,
octet pour octet, sur chaque réseau que Vela prend en charge.
