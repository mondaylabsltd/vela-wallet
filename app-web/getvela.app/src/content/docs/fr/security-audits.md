---
title: Audits et problèmes connus
description: Chaque contrat on-chain dont Vela dépend, qui l'a audité, si la version auditée correspond à celle qui est déployée, et ce qui n'est pas audité du tout.
---

« Audité » est une affirmation sur une version précise d'un code précis : cette
page ne se contente donc pas d'agiter le mot — elle cite les rapports exacts, les
adresses de déploiement exactes et les écarts entre versions auditée et déployée.
Elle liste aussi ce qui n'est _pas_ audité, parce que cette liste-là porte autant
de poids que la première.

Dernière relecture : août 2026. Si vous trouvez une erreur ici, dites-le-nous,
nous la corrigerons.

## Le chemin des fonds

Quatre couches de contrats peuvent toucher votre argent. Les quatre sont des
contrats tiers avec des audits publiés, et dans chaque cas l'adresse déployée est
le déploiement canonique officiel.

### Safe v1.4.1 — le compte lui-même

Votre portefeuille est un proxy
[Safe](https://github.com/safe-global/safe-smart-account) : singleton SafeL2,
fabrique de proxys, gestionnaire de repli de compatibilité, et MultiSend pour les
lots.

[Ackee Blockchain a audité Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(rapport final mars 2023) : 11 constats, aucun critique ni élevé. Le v1.4.1 que
nous déployons diffère du v1.4.0 audité par une correction de compatibilité
ERC-4337 d'une seule ligne
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). La logique
de MultiSend est inchangée depuis le
[v1.3.0 audité par G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Toutes les adresses correspondent aux déploiements canoniques de
[safe-deployments](https://github.com/safe-global/safe-deployments), et les
contrats entrent dans le
[bug bounty de la Safe Foundation](https://docs.safefoundation.org/security/bug-bounty)
(jusqu'à 1 000 000 $ pour un constat critique).

Ce qu'un audit ne couvre pas : l'incident Bybit de 2025. Cette attaque a compromis
la chaîne de build du frontend web officiel de Safe, pas les contrats — la
[conclusion officielle de l'enquête](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
n'a trouvé aucune vulnérabilité dans les contrats Safe. Nous la lisons comme une
leçon sur la couche web et opérationnelle, celle-là même sur laquelle vous devriez
nous examiner aussi.

### Safe4337Module v0.3.0 — l'adaptateur ERC-4337

Déployé à `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, l'adresse canonique de
v0.3.0 (correspondance exacte Sourcify — le bytecode on-chain est le code audité).
[Audité par Ackee Blockchain](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(rapport final mars 2024), sans constat non résolu au-dessus du niveau informatif.
La combinaison v0.3.0 + EntryPoint v0.7 + Safe ≥ 1.4.1 que nous utilisons est
exactement la configuration décrite par l'audit et les notes de version.

L'historique du module comporte un problème divulgué : v0.1.0 (2023) ne signait
pas `initCode` ni `paymasterAndData`, un vecteur de griefing de gas. Il a été
[corrigé en v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module),
et v0.1.0 n'a jamais quitté les testnets. Nous utilisons v0.3.0, qui hérite du
correctif.

### SafeWebAuthnSharedSigner v0.2.1 — le signataire de passkey

Déployé à `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, l'adresse canonique de
v0.2.1 (la même sur chaque chaîne via la fabrique de singletons de Safe).

Ce que « shared » veut dire — et ne veut pas dire : c'est le _déploiement du
contrat_ qui est partagé, comme le singleton Safe l'est. Votre clé, non. Chaque
Safe appelle `configure()` par delegatecall et stocke sa propre clé publique P-256
dans son propre stockage. Une instance de signataire représente exactement une
passkey par Safe, et le Safe de quelqu'un d'autre ne peut pas utiliser la vôtre.

La version compte ici. L'audit de v0.2.0
[indiquait explicitement](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
que le signataire partagé était hors périmètre — le contrat n'existait pas encore.
Les audits qui couvrent ce que nous déployons sont ceux de v0.2.1 : un
[concours d'audit Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(juin–juillet 2024 : zéro élevé, zéro moyen, trois constats faibles — tous
corrigés) plus une
[revue Certora du commit de publication](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
sans nouveau constat. Aucune vulnérabilité au niveau du contrat n'a été divulguée
depuis la sortie ; les contrats de passkey entrent dans le bounty de la Safe
Foundation.

La documentation de Safe recommande d'associer la propriété par passkey à un
chemin de récupération plutôt que de traiter un identifiant unique comme la seule
clé du compte. La façon dont Vela s'y prend est documentée dans
[récupération et connexion](/fr/docs/recovery).

La vérification P-256 on-chain utilise directement le précompilé RIP-7212, sans
vérificateur Solidity de repli. Avant d'activer un réseau, l'application sonde le
précompilé avec une vraie signature et refuse le réseau si la vérification échoue.
Deux réserves honnêtes : la spécification RIP-7212 d'origine a des défauts sur des
cas limites que l'[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) est venu
corriger (ils n'affectent pas des signatures WebAuthn bien formées), et un sondage
ne peut pas attraper toutes les façons dont l'implémentation d'une chaîne pourrait
diverger dans des contextes d'exécution inhabituels.

### EntryPoint v0.7 — le point d'entrée ERC-4337

Déployé à `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, le
[déploiement canonique de v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Audité par OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(commandé par l'Ethereum Foundation, janvier 2024) : zéro critique, zéro élevé,
cinq constats moyens, tous résolus — et le commit audité est la version déployée.
EntryPoint v0.7.0 entre dans le
[bug bounty ERC-4337](https://docs.erc4337.io/community/bug-bounty) de l'Ethereum
Foundation (jusqu'à 250 000 $).

## Problèmes connus que nous surveillons

### Le vecteur de griefing de l'EntryPoint

En février 2026, des chercheurs de Trust Security ont
[divulgué](https://erc4337.substack.com/p/improving-useroperation-execution)
un vecteur de griefing et de censure touchant tous les EntryPoint antérieurs à
v0.9, dont le v0.7 que nous utilisons. Un attaquant qui intercepte une
UserOperation signée avant son inclusion peut l'exécuter dans un cadre d'appel
qu'il contrôle et forcer l'exécution interne à échouer — l'opération échoue, mais
le gas est quand même facturé. L'Ethereum Foundation a versé 50 000 $ de prime aux
chercheurs ; elle a classé le problème comme vecteur de censure/griefing, pas de
vol de fonds, et il n'a jamais été exploité.

Ce qu'il peut faire : gâcher des frais et retarder une transaction. Ce qu'il ne
peut pas faire : voler des fonds ou falsifier une signature. L'exposition de Vela
est étroite parce que les UserOperations vont droit à un relais plutôt que par un
mempool public — il y a donc peu d'occasions d'en intercepter une — et le pire cas
est borné par les frais que vous avez déjà acceptés. Le correctif n'existe que
dans EntryPoint v0.9 (novembre 2025) ; v0.7 lui-même n'est pas corrigeable. Nous
comptons migrer à mesure que l'écosystème autour — en particulier la lignée du
module 4337 de Safe — prendra en charge v0.9, et nous le noterons ici.

## Ce qui n'est pas audité

- **Les contrats de Vela.** Deux petits contrats que nous avons écrits, déployés
  sur Gnosis : l'
  [index de clés publiques de passkeys](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (un registre en ajout seul qui aide vos appareils à trouver votre clé publique)
  et son assistant de lots. Ils ne sont pas audités. Par construction, ils ne
  détiennent aucun fonds, n'ont pas de propriétaire et ne peuvent pas être mis à
  niveau — c'est une couche de découverte, pas une couche d'autorisation. Le
  pouvoir de dépense vient toujours de la passkey configurée dans votre Safe. Le
  pire échec réaliste est du griefing (quelqu'un squatte une entrée d'index), ce
  qui peut rendre la récupération moins pratique mais ne peut pas déplacer
  d'argent. Un contrat de répartition du règlement du gas, issu d'une ancienne
  conception des frais, ne fait plus partie du flux de transaction.
- **Multicall3.** Son propre README le
  [dit clairement](https://github.com/mds1/multicall3) : « This contract is
  unaudited. » Nous l'utilisons exactement comme ses auteurs le décrivent comme
  sûr — des appels de lecture groupés pour les soldes, les métadonnées de tokens
  et les prix. Vela ne lui accorde jamais d'approbations et il ne détient jamais de
  fonds. Le pire cas d'un bug est une lecture incorrecte.
- **Le déployeur CREATE2.** Le
  [proxy de déploiement déterministe d'Arachnid](https://github.com/Arachnid/deterministic-deployment-proxy)
  est le déployeur sans état standard de l'écosystème ; il n'a pas d'audit formel.
  Nos vérifications réseau échouent par sécurité s'il est absent ou modifié sur une
  chaîne.
- **Tempo et pathUSD.** Tempo, l'un de nos douze réseaux intégrés, n'a pas de pièce
  native ; le gas y est réglé dans le stablecoin pathUSD. En août 2026, ni le
  protocole de base de Tempo ni pathUSD n'ont d'audit de sécurité publié ni de bug
  bounty, et une
  [évaluation indépendante des collatéraux par DefiLlama](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (avril 2026) a classé pathUSD à haut risque. C'est un risque au niveau de la
  chaîne qu'aucun portefeuille ne peut atténuer : les fonds que vous détenez sur
  Tempo, et le règlement du gas là-bas, en héritent. Traitez Tempo comme la chaîne
  la plus récente et la moins éprouvée de la liste et dimensionnez vos soldes en
  conséquence. Nous mettrons cette section à jour à mesure que des audits
  paraîtront.
- **Vela lui-même.** Notre application et nos services backend n'ont pas eu
  d'audit tiers. C'est la plus grande réserve de cette page, nous l'indiquons dans
  l'en-tête du site, et les détails honnêtes sont dans
  [Vela est en alpha](/blog/vela-is-in-alpha). Commencez petit. Lisez le code.

## Vérifiez par vous-même

Chaque adresse ci-dessus est un déploiement public canonique que vous pouvez
comparer aux registres officiels —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
et les
[notes de version de l'EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) :

| Contrat | Adresse |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Index de clés publiques de passkeys (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
