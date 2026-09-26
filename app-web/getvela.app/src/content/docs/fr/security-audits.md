---
title: Audits et problèmes connus
description: "Chaque contrat dont Vela dépend, qui a audité quelle version, si la version auditée est bien celle qui est déployée, les constats ouverts que nous surveillons, et ce qui n'a pas été audité du tout."
source: 377855411c74
---

« Audité » est une affirmation qui porte sur un code précis, dans une version
précise : cette page cite donc les rapports, les commits et les adresses déployées
— et liste ce qui n'est **pas** audité, ce qui compte tout autant.

Dernière vérification : 22 septembre 2026. Si vous trouvez une erreur,
signalez-la-nous et nous la corrigerons.

## Le chemin des fonds

Chaque contrat qui peut toucher à votre argent est un déploiement canonique de code
tiers ayant fait l'objet de revues publiées.

### Safe v1.4.1 — le compte lui-même

Votre portefeuille est un proxy
[Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) qui utilise le
singleton SafeL2 et la SafeProxyFactory. Les lots passent par MultiSend.

[Ackee Blockchain a audité Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(rapport final le 16 mars 2023, revue des correctifs le 28 mars) : 11 constats,
aucun critique ni élevé ; les deux constats moyens ont été reconnus plutôt que
corrigés. Le périmètre couvrait SafeL2, SafeProxyFactory,
CompatibilityFallbackHandler, MultiSendCallOnly et SignMessageLib. La v1.4.1 ne
diffère de la v1.4.0 que par une ligne fonctionnelle, un correctif de compatibilité
ERC-4337 dans la configuration des modules
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)) ; Safe a
consulté Ackee et conclu qu'un nouvel audit n'était pas nécessaire. La logique de
MultiSend n'a pas changé depuis la v1.3.0, qu'[a auditée G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Toutes les adresses correspondent à
[safe-deployments](https://github.com/safe-global/safe-deployments). Les contrats
centraux entrent dans le périmètre du
[programme de bug bounty de la Safe Foundation](https://docs.safefoundation.org/security/bug-bounty),
dont le palier le plus élevé paie jusqu'à 1 000 000 $.

L'incident de Bybit en 2025 n'est pas un constat sur les contrats : les attaquants
ont altéré le JavaScript servi à l'interface web de Safe, et la
[déclaration d'analyse forensique](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
de Safe n'a relevé aucune vulnérabilité dans les contrats.
[Notre page sur le sujet](/fr/docs/bybit-attack) explique pourquoi la même classe
d'attaque concerne toute interface de portefeuille, la nôtre comprise.

### Safe4337Module v0.3.0 — l'adaptateur ERC-4337

Déployé à `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (correspondance exacte sur
Sourcify), et également défini comme fallback handler de votre Safe. Revu trois
fois — [rapports ici](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md) :

- **Ackee Blockchain**, rapport final en mars 2024 : un avertissement (utilisation
  de l'optimiseur du compilateur) reconnu, rien de plus grave en suspens.
- **Certora**, août 2026 : un constat **moyen**, reconnu mais **non corrigé**
  dans la v0.3.0 — *les changements d'autorisation n'invalident pas les
  UserOperations suivantes déjà validées dans le même lot*. Voir « Problèmes connus »
  plus bas.
- **Nethermind**, août 2026 : aucun constat.

SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), qui active le module au déploiement d'un
portefeuille, était couvert par les revues de Certora et de Nethermind.

L'historique du module compte un problème divulgué : la v0.1.0 ne signait pas
`initCode` ni `paymasterAndData`, un vecteur de gas griefing
[corrigé dans la v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module) ;
selon Safe, la v0.1.0 n'a pas été utilisée en dehors des testnets. Vela utilise la
v0.3.0 avec l'EntryPoint v0.7 et Safe 1.4.1, la configuration décrite dans la
version publiée du module.

### Module passkey de Safe v0.2.1 — les signataires

Votre première clé est vérifiée par **SafeWebAuthnSharedSigner** à
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`. « Partagé » signifie que le
déploiement du contrat est partagé, comme l'est le singleton Safe ; votre clé, elle,
ne l'est pas. Chaque Safe stocke sa propre clé publique P-256 dans son propre
stockage.

Chaque clé supplémentaire a son propre contrat signataire, créé par
**SafeWebAuthnSignerFactory** à `0x1d31F259eE307358a26dFb23EB365939E8641195`
comme proxy vers le **singleton SafeWebAuthnSigner** à
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`.

Les revues qui couvrent ces contrats en v0.2.1
([rapports](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)) :

- Une [compétition d'audit Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (juin–juillet 2024) : aucun constat élevé ni moyen ; trois faibles, tous corrigés.
- La revue de **Certora** sur le commit de publication : aucun nouveau constat.
  (L'audit antérieur de la v0.2.0 précise que le signataire partagé n'avait pas
  encore été audité — il a été ajouté après cet audit.)
- **Nethermind**, août 2026 : aucun constat.

Aucune vulnérabilité au niveau des contrats n'a été divulguée depuis la
publication, et les contrats passkey entrent dans le périmètre du bug bounty de la
Safe Foundation.

Les signatures de passkey sont vérifiées par le précompilé **EIP-7951 / RIP-7212** de la
chaîne, sans vérificateur de repli. Avant d'activer un réseau, l'app contrôle le
précompilé avec une vraie signature. Deux réserves : la spécification RIP-7212
d'origine présente des défauts dans des cas limites que
l'[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) corrige (ils ne concernent
que des entrées qui doivent échouer de toute façon, pas les signatures WebAuthn
bien formées), et un test ne peut pas détecter toutes les façons dont
l'implémentation d'une chaîne pourrait diverger.

### EntryPoint v0.7 — exécute votre opération

Déployé à `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, la
[version canonique v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Audité par OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
pour l'Ethereum Foundation (janvier 2024) : aucun constat critique ni élevé, cinq
moyens, et les 24 constats résolus ; le commit de la revue des correctifs
correspond à la version publiée. Il entre dans le périmètre du
[bug bounty ERC-4337](https://docs.erc4337.io/community/bug-bounty) de l'Ethereum
Foundation (jusqu'à 250 000 $).

## Les problèmes connus que nous surveillons

### Changements d'autorisation au sein d'un même lot (Safe4337Module, Certora M-01)

L'EntryPoint valide toutes les opérations d'un lot avant d'exécuter la première.
Donc, si une opération retire un propriétaire, une opération signée par ce
propriétaire et placée plus loin dans le même lot passe quand même la validation
et s'exécute. Safe a reconnu le problème et n'a pas modifié la v0.3.0.

Les apps Vela ne construisent jamais de changement de propriétaires, et une dApp
qui en demande un est refusée d'emblée : Vela lui-même ne déclenche donc jamais ce
cas. Cela compte encore pour quiconque retire une clé compromise via un autre outil
Safe : cette personne ne pourrait pas compter sur la neutralisation de cette clé dans
le même lot.

### Interception d'une opération signée (EntryPoint avant la v0.9)

En février 2026, des chercheurs ont
[divulgué](https://erc4337.substack.com/p/improving-useroperation-execution) un
vecteur de griefing et de censure qui touche tous les EntryPoint antérieurs à la
v0.9, dont la v0.7. Quelqu'un qui obtient une opération signée avant son inclusion
peut l'exécuter à l'intérieur d'un appel qu'il contrôle et forcer l'échec de
l'exécution interne : l'opération échoue et doit être signée à nouveau. (Avec les
frais intégrés à l'opération propres à Vela, le transfert des frais est annulé avec
elle : c'est donc le relais, et non vous, qui absorbe le gas.) Cela concerne les
opérations qui appellent des contrats protégés contre la réentrance ou qu'un état
temporaire peut faire échouer ; les simples transferts ne sont pas concernés.
Utilisé de façon répétée contre des flux de retrait, ce vecteur pourrait rendre
des fonds indisponibles pendant un certain temps. Il ne peut ni falsifier une
signature ni détourner des fonds.

Le relais de Vela soumet les opérations directement plutôt que via une mempool
partagée, mais une transaction `handleOps` en attente reste visible dans la mempool
publique : cela réduit l'exposition sans la supprimer. Le correctif n'existe que
dans l'EntryPoint v0.9 (novembre 2025) ; la v0.7 ne peut pas être corrigée. La
migration dépend de la prise en charge de la v0.9 par le module 4337 de Safe, et
cette page dira quand elle aura lieu.

### Lacunes dans les défenses de Vela

Ce ne sont pas des constats sur les contrats, mais des endroits où le portefeuille
vous protège moins que vous ne pourriez le croire. Chacun est suivi en vue d'une
correction, sauf là où il est indiqué qu'il s'agit d'un compromis délibéré :

- **Une approbation illimitée part si vous la gardez** — un compromis délibéré, car
  une approbation plafonnée casse Permit2 et les swaps groupés. Une approbation
  « illimitée » (2^200 ou plus ; 2^152 pour Permit2) s'affiche en rouge et est
  envoyée telle que la dApp l'a demandée, sauf si vous la plafonnez. Les permits
  signés ne peuvent être plafonnés nulle part.
- **La page de signature indépendante n'est encore reliée** à aucune app.
- **Le site web charge un script d'analyse d'audience tiers** sur le même domaine
  que les passkeys. Le site interdit à ses pages d'utiliser les passkeys (un en-tête
  Permissions-Policy), et tient ce script à l'écart de la page qui détient une clé.

## Ce qui n'est pas audité

- **Les contrats de Vela.** Le
  [registre des clés publiques](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  à `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis ; la même adresse sur
  Ethereum et Base), le déploiement d'origine du registre à
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (son adresse fait partie du domaine
  de signature de chaque enregistrement), et l'ancien index qu'ils ont remplacé
  (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, historique en lecture seule). Ils
  ne sont pas audités. Ils ne détiennent aucun fonds, n'ont pas de propriétaire et
  ne peuvent pas être mis à niveau ; c'est une couche de découverte, pas une couche
  d'autorisation. Le pouvoir de dépense ne vient que des clés configurées dans votre
  Safe. La pire défaillance réaliste, c'est qu'un portefeuille devienne plus
  difficile à retrouver sur un nouvel appareil, pas que de l'argent bouge.
- **Multicall3.** Son README
  [indique](https://github.com/mds1/multicall3) « This contract is unaudited » (ce
  contrat n'est pas audité). Vela ne l'utilise que pour des lectures groupées —
  soldes, informations sur les jetons, cotations de prix —, jamais avec des
  approbations ou des fonds.
- **Les déployeurs déterministes** (le proxy CREATE2 d'Arachnid et la singleton
  factory de Safe) — des standards de l'écosystème, sans état, qui n'ont pas fait
  l'objet d'audits formels. La vérification des réseaux de Vela échoue par défaut
  s'ils sont absents ; elle vérifie que du code existe à l'adresse, pas qu'il
  correspond octet pour octet.
- **Tempo.** L'un des 24 réseaux intégrés, sans monnaie native ; Vela y paie le gas
  en stablecoin pathUSD. En septembre 2026, la
  [politique de sécurité](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)
  de Tempo indique que le protocole est encore en cours d'audit et n'a pas de bug
  bounty actif. Les fonds détenus sur Tempo, et le gas payé sur ce réseau, portent
  ce risque au niveau de la chaîne ; considérez-le comme la chaîne la plus récente
  et la moins éprouvée de la liste.
- **Vela lui-même.** Les apps, les services backend et les contrats ci-dessus n'ont
  fait l'objet d'aucun audit tiers, et aucun n'est programmé. C'est la plus grande
  réserve de cette page. Les détails sont dans
  [Vela is in alpha](/blog/vela-is-in-alpha). Commencez avec de petits montants, et
  lisez le code.

## Vérifiez par vous-même

Chaque adresse ci-dessous est un déploiement public canonique. Vérifiez-les avec
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
et la [version publiée de l'EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) :

| Contrat                                   | Adresse                                      |
| ----------------------------------------- | -------------------------------------------- |
| Singleton SafeL2 v1.4.1                   | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| Singleton SafeWebAuthnSigner v0.2.1       | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Registre des clés publiques (Vela, non audité) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Vérifié à l'ajout d'un réseau ; votre Safe utilise à la place le module 4337
comme fallback handler.
