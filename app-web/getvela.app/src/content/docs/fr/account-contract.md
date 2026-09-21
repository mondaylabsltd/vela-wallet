---
title: Le contrat de compte
description: "Votre portefeuille Vela est un Safe v1.4.1 non modifié. Aucun contrat sur le chemin de votre argent n'a été écrit par Vela — voici exactement de quels contrats il s'agit, ce que cela vous apporte, et ce que cela coûte."
source: 588a6ba6e672
---

# Le contrat de compte

Votre portefeuille n'est pas une structure de données privée d'une app. C'est un
compte intelligent **Safe v1.4.1** — le contrat qu'utilisent de nombreuses grandes
trésoreries on-chain —, déployé exactement tel que Safe le publie, sans aucune
modification.

## Rien sur le chemin n'est à nous

Chaque contrat qui peut toucher à votre argent a été écrit par Safe ou par les
auteurs d'ERC-4337 :

| Contrat | Rôle dans votre portefeuille | Écrit par |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, via un proxy) | Le compte lui-même ; propriétaires, seuil, exécution | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Permet à l'EntryPoint d'actionner le Safe ; sert aussi de fallback handler | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Vérifie les signatures P-256 de la première clé | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) et les signataires qu'elle crée | Un petit contrat signataire par clé supplémentaire | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Exécute votre opération signée | Auteurs d'ERC-4337 |

Les contrats de Vela ne figurent pas dans cette liste : le **registre des clés
publiques**, qui enregistre les clés de chaque portefeuille pour qu'un nouvel
appareil puisse le retrouver ([récupération](/fr/docs/recovery)), le registre de
domaines qui l'accompagne, et l'ancien index qu'ils ont remplacé. Ils ne
détiennent aucun fonds et n'ont aucun rôle dans votre Safe.

Le dépôt du portefeuille ne contient pas une seule ligne de Solidity — vous pouvez
le vérifier en une commande :

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # n'affiche rien
```

Vela ne détient aucun rôle privilégié sur votre compte : pas de clé
d'administration, pas de chemin de mise à niveau, aucun module qu'il puisse
ajouter. Seules vos clés peuvent modifier votre Safe.

## Pourquoi « non modifié » est le mot qui compte

Beaucoup de portefeuilles sont bâtis sur « un Safe », un fork de Safe, ou un compte
inspiré de Safe. La différence compte de trois façons.

**Les audits portent sur ce que vous utilisez réellement.** Les audits de Safe
couvrent ces versions, ou des versions antérieures dont elles ne diffèrent que par
de petites modifications documentées (la [page des audits](/fr/docs/security-audits)
donne les détails). Les audits d'un fork portent sur le code d'avant le fork ; la
modification, elle, n'a été auditée par personne.

**L'écosystème traite votre compte comme un Safe, parce que c'en est un.** Les
explorateurs de blocs le décodent, et les outils de Safe peuvent le lire et
construire des transactions pour lui. Pour *signer* ces transactions, en revanche,
un programme doit pouvoir demander à votre clé une signature pour `getvela.app`, le
domaine auquel appartiennent vos passkeys — l'app web de Safe, servie depuis un
autre domaine, ne peut donc pas signer à votre place. Le
[guide d'auto-hébergement](/fr/docs/self-hosting#if-getvela-app-disappears) liste ce
qui le peut.

**La surface d'attaque est surveillée par beaucoup d'autres.** Un contrat de compte
sur mesure est surveillé surtout par son auteur. Les contrats centraux de Safe sont
surveillés par tous ceux qui gardent de l'argent dans un Safe ; les modules 4337 et
passkey ont un public plus restreint, mais bien réel.

## Ce que cela coûte

Être standard n'est pas gratuit :

- **Le gas.** Votre signature est vérifiée on-chain et la transaction passe par
  l'EntryPoint. Un simple envoi depuis un portefeuille Vela déployé a consommé
  environ 140 000 à 170 000 gas on-chain lors de nos mesures sur Gnosis (septembre
  2026) ; un transfert d'ETH ordinaire depuis un compte classique en consomme
  21 000. En plus du gas, le relais prélève ses frais — voir
  [réseaux et frais](/fr/docs/networks-and-fees).
- **Le compte doit être déployé.** Votre adresse est calculée avec `CREATE2` avant
  que quoi que ce soit n'existe on-chain : vous pouvez donc y recevoir des fonds
  immédiatement ; votre première transaction sortante sur chaque réseau paie le
  déploiement du contrat.
- **Toutes les chaînes ne conviennent pas.** Les signatures de passkey sont
  vérifiées par le précompilé **RIP-7212**, dont l'adresse fait partie des données
  de configuration de chaque portefeuille : un réseau qui ne l'a pas ne peut pas
  faire fonctionner Vela du tout.
- **Le risque de Safe devient le vôtre.** Faire confiance à un contrat très utilisé
  reste faire confiance à un contrat. Vela n'a pas ajouté, sur le chemin de votre
  argent, un second contrat maison auquel vous devriez aussi vous fier.

## Ce qui est audité, et ce qui ne l'est pas

Les contrats de Safe, ses modules 4337 et passkey, et l'EntryPoint v0.7 ont fait
l'objet d'audits tiers publiés. **Le code de Vela — les apps, les services backend
et le contrat de registre — n'a fait l'objet d'aucun audit tiers, et aucun n'est
programmé** ; c'est un objectif pour le jour où le projet pourra en financer un,
pas un engagement daté. Chaque contrat, son rapport d'audit et les problèmes que
nous suivons sont dans [audits et problèmes connus](/fr/docs/security-audits).

## Voyez par vous-même

Votre compte est on-chain. Une fois qu'il est déployé, ouvrez votre adresse dans un
explorateur de blocs : c'est un proxy Safe dont l'implémentation est le déploiement
canonique SafeL2 v1.4.1 de Safe, sur tous les réseaux.

Ensuite : [audits et problèmes connus](/fr/docs/security-audits).
