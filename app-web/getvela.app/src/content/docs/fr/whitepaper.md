---
title: Livre blanc
description: Comment Vela fonctionne et ce que vous devez — et ne devez pas — croire sur parole pour l'utiliser. Architecture, modèle de sécurité, récupération, et comment tout vérifier vous-même.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Livre blanc

<Callout type="info" title="État : alpha · v0.1">
Cette page décrit comment Vela fonctionne aujourd'hui, et ce que vous devez ou ne
devez pas croire sur parole pour l'utiliser. Elle préfère l'honnêteté au marketing.
Vela est en <a href="/blog/vela-is-in-alpha">alpha</a> — commencez petit. Vela n'a
pas de token. Tout ici est vérifiable face au code open source.
</Callout>

## Résumé

Vela est un **portefeuille à contrat intelligent auto-conservé** pour les réseaux
EVM. Chaque portefeuille est un compte intelligent
[Safe](https://github.com/safe-fndn/safe-smart-account) contrôlé par une
**passkey** — un identifiant WebAuthn (P-256) gardé par le système de votre
appareil, chiffré de bout en bout, déverrouillé par Face ID, Touch ID ou une
empreinte. Il n'y a ni phrase de récupération ni clé privée à copier, stocker ou
perdre.

Vela, l'entreprise, ne détient jamais vos clés ni vos fonds et **ne peut ni les
déplacer, ni les geler, ni les saisir**. L'application, le relais de transactions
et les services associés sont tous open source et auto-hébergeables. Ce que vous
devez croire se réduit à des contrats audités, au coffre de passkeys de votre
système d'exploitation et — pour la seule disponibilité — à un relais que vous
pouvez remplacer ou faire tourner vous-même.

## Pourquoi Vela existe

La plupart des portefeuilles imposent un compromis :

- **Les portefeuilles à phrase de récupération** mettent un secret de 12 à 24 mots
  devant chaque utilisateur. C'est le point de défaillance unique et une cible
  permanente d'hameçonnage.
- **Les portefeuilles dépositaires** suppriment la phrase mais prennent la garde
  de vos fonds — réintroduisant le risque de contrepartie que la crypto devait
  supprimer.
- **La signature à l'aveugle** — approuver de l'hexadécimal illisible — s'est
  normalisée dans tout l'écosystème et se trouve derrière une grande part des
  portefeuilles vidés.

Vela vise à être aussi simple qu'une application dépositaire tout en vous laissant
pleinement auto-conservé : pas de phrase, pas de garde par un tiers, et aucune
transaction que vous ne pouvez lire avant de la signer.

## Principes de conception

1. **Auto-conservation, sans exception.** Les clés sont générées sur votre
   appareil et gardées par le service de passkeys de votre système, chiffrées de
   bout en bout. Les serveurs de Vela ne voient que des données publiques.
2. **Vérifier, pas croire.** Toute la pile — l'application et les quatre services
   backend — est open source sous licence MIT.
3. **Pas de signature à l'aveugle.** Les transactions sont traduites en intention
   lisible partout où un descripteur existe ; les appels inconnus sont signalés,
   pas cachés.
4. **En faire moins.** Le portefeuille détient de l'ETH et des ERC-20 et se
   connecte aux dApps que vous choisissez. Moins de code à croire, une surface
   d'attaque plus petite.

## Architecture

```text
Application Vela (iOS / Android / Web, une seule base de code)
  • Passkey (WebAuthn P-256, service de passkeys du système)
  • Construction et signature de l'UserOperation
  • Interface de signature lisible (ERC-7730)
        │  UserOperation signée
        ▼
Relais Vela (ERC-4337, auto-hébergeable)
  • soumet handleOps à l'EntryPoint
  • ne peut ni altérer ni falsifier votre transaction
        ▼
Chaîne EVM
  EntryPoint v0.7 → compte intelligent Safe
  Le signataire WebAuthn vérifie P-256 on-chain
```

### Modèle de compte

Votre portefeuille est un compte intelligent **Safe v1.4.1** (un contrat proxy)
opéré via l'abstraction de compte **ERC-4337** (EntryPoint v0.7) avec le **Safe
4337 Module** et un **signataire WebAuthn** comme propriétaire du compte.

L'adresse est **déterministe** et **contrefactuelle** : elle est calculée à partir
de la clé publique de votre passkey via `CREATE2` avant qu'aucune transaction ne
soit envoyée, vous pouvez donc recevoir des fonds avant tout déploiement. Le compte
se déploie lui-même, payé sur son propre solde, à votre première transaction.

### Clés et authentification

L'authentification utilise des **passkeys WebAuthn** sur la courbe **P-256**. La
clé privée est générée sur votre appareil et gardée, chiffrée de bout en bout, par
le service de passkeys de votre système (trousseau iCloud ou gestionnaire de mots
de passe Google), qui la synchronise entre vos appareils. **Les serveurs de Vela ne
voient jamais que votre clé publique.** Chaque signature exige une vérification
biométrique fraîche — il n'y a pas de clé de session longue durée. Tous les détails
dans [comment fonctionnent les passkeys](/fr/docs/passkeys).

### Signature et flux de transaction

1. **Construire** une `UserOperation` ERC-4337 pour votre Safe et estimer le gas.
2. **Décoder** l'appel en intention lisible et l'afficher pour vérification.
3. **Signer** — votre appareil produit une assertion WebAuthn sur le hash de
   l'opération après vérification biométrique.
4. **Encoder** l'assertion en signature de contrat **EIP-1271**.
5. **Relayer** l'opération signée vers le relais, qui la soumet à l'EntryPoint.
6. **Vérifier on-chain** — le Safe vérifie la signature P-256 on-chain via le
   précompilé RIP-7212 avant d'exécuter. Ce précompilé est une exigence stricte :
   il n'y a pas de vérificateur de repli, et Vela refuse d'activer un réseau qui en
   manque.

Le relais reçoit une opération **déjà signée**. Il ne peut changer ni le
destinataire, ni le montant, ni aucun autre champ sans invalider la signature.

### Relais et modèle de gas

- Le gas est payé **sur le solde de votre propre portefeuille** — dans le token
  natif du réseau par défaut, ou dans un stablecoin pris en charge là où le relais
  en propose un. Tempo, sans pièce native, règle toujours le gas en stablecoins
  USD. Il n'y a **pas de paymaster** ni de tiers qui sponsorise — ou conditionne —
  vos transactions.
- **Le relais est la source unique de vérité pour le prix du gas.** Il l'annonce à
  partir des conditions réelles de la chaîne ; le portefeuille affiche cette
  annonce et signe exactement ce qu'il affiche.
- La facturation du relais de Vela est délibérément simple : le total, ce sont les
  **coûts réseau plus les frais de service du relais**, avec un petit minimum sur
  les transactions très bon marché. Une part va aux validateurs de la chaîne ; le
  reste rémunère le relais qui fait tourner l'infrastructure et maintient votre
  compte de gas approvisionné.
- Le portefeuille **affiche les frais estimés avant votre confirmation** — dans
  l'actif de frais et dans votre devise d'affichage — et le montant annoncé comme
  son destinataire font partie de ce que vous signez : le relais est payé exactement
  ce qui était affiché. Aucune marge cachée.
- Chaque Safe a un **compte de relais dédié** (compte de gas) par chaîne, activé
  par un dépôt **non remboursable**. Il peut se vider avec le temps, et donc
  demander une **réactivation** plus tard — ce n'est pas strictement un dépôt
  unique.

Le relais est une dépendance de **disponibilité**, pas de **garde** : il peut
retarder ou refuser, mais jamais altérer, falsifier ou voler. Il est open source et
vous pouvez faire tourner le vôtre — et comme le prix est **annoncé et affiché**
plutôt que caché, même les frais d'un relais auto-hébergé ou tiers vous sont
toujours visibles avant de signer. Voir
[réseaux et frais](/fr/docs/networks-and-fees).

### Signature lisible (ERC-7730)

Vela décode la calldata et les données typées EIP-712 à l'aide de descripteurs
**ERC-7730** et affiche l'**intention** (Échanger, Envoyer, Approuver…), la
**substance** (montants, adresses) et, à la demande, les **détails** (nonce,
échéance, calldata brute), colorés selon le risque. Quand aucun descripteur ne
correspond, Vela affiche un avertissement explicite de signature à l'aveugle plutôt
que de faire semblant de comprendre l'appel.

### Réseaux

Vela prend en charge 12 réseaux EVM — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad et World Chain — plus les
réseaux personnalisés. Un réseau personnalisé ne peut être ajouté que s'il héberge
déjà les contrats dont Vela dépend (l'EntryPoint, les contrats Safe, le signataire
WebAuthn) et le précompilé P-256 RIP-7212 ; Vela le vérifie avant activation.

## Modèle de sécurité

**Ce que Vela ne peut pas faire :**

- Déplacer, dépenser ou transférer vos fonds — seule votre passkey peut autoriser
  le Safe.
- Geler ou saisir votre compte — le Safe est votre contrat on-chain ; Vela n'y a
  aucun rôle privilégié.
- Signer à votre place — chaque transaction exige une assertion biométrique
  fraîche.
- Voir votre clé privée — elle n'atteint jamais Vela ; seul votre appareil peut
  s'en servir pour signer.
- Modifier une transaction après votre signature — tout changement l'invalide.

**Ce que « ne peut pas geler » ne couvre pas : le *token*.** Un stablecoin à
permissions — USDC, USDT et la plupart des tokens adossés à une monnaie — porte une
fonction de liste noire que son émetteur peut appeler contre n'importe quelle
adresse, y compris la vôtre. Ce pouvoir appartient à l'émetteur et existe quel que
soit le portefeuille dans lequel vous détenez le token ; aucun portefeuille
auto-conservé, Vela compris, ne peut le retirer. Ce que l'auto-conservation vous
donne, c'est que **nous** ne sommes pas une seconde partie qui le pourrait.

**Ce à quoi vous faites confiance :**

- Les **contrats Safe** (audités, largement utilisés) et le signataire WebAuthn qui
  vérifie votre clé P-256.
- Le **service de passkeys de votre système** (Apple / Google) pour protéger et
  synchroniser votre identifiant.
- Les **fournisseurs RPC** que vous interrogez (Vela utilise un pool multi-sources
  avec bascule ; vous pouvez définir les vôtres).
- Le **relais**, pour la seule disponibilité — et vous pouvez l'auto-héberger.

**Menaces prises en compte :**

- **Appareil perdu ou volé** — un voleur a encore besoin de votre biométrie ou de
  votre code pour signer.
- **Hameçonnage / dApp malveillante** — traité par la signature lisible.
- **Serveur Vela compromis** — n'apporte aucune capacité de signature ; le rayon
  d'impact est un service dégradé, pas une perte de fonds.
- **Risque de chaîne d'approvisionnement** — atténué par l'open source et
  l'auto-hébergement.

## Récupération

Votre passkey est sauvegardée par le service de votre système ; sur un nouvel
appareil, se connecter au même compte Apple ou Google la restaure, et votre
portefeuille réapparaît.

<Callout type="warning" title="La sauvegarde de passkeys de votre plateforme est votre récupération">
La récupération de Vela, c'est votre passkey, synchronisée par le trousseau iCloud
ou le gestionnaire de mots de passe Google. Par conception, il n'y a pas de phrase
de récupération, pas de récupération sociale, pas de gardiens — rien que Vela
pourrait perdre, laisser fuir ou être contraint d'utiliser. Le revers est réel : si
vous perdez <strong>à la fois</strong> votre appareil <strong>et</strong> la
passkey synchronisée dans le cloud, sans autre copie, le compte ne peut pas être
récupéré. Gardez la sauvegarde de passkeys de votre plateforme activée et ce compte
sécurisé.
</Callout>

Le modèle complet de récupération, limites honnêtes comprises, est dans
[récupération et connexion](/fr/docs/recovery).

## Si Vela disparaît

L'auto-conservation signifie que vos clés et vos fonds ne dépendent pas de la
présence de Vela en ligne. Les fonds vivent dans **votre contrat Safe on-chain**,
et le relais est open source et remplaçable.

Une réserve honnête : WebAuthn lie une passkey à un domaine de relying party
(`getvela.app`). Si ce domaine était perdu définitivement, les passkeys qui y sont
liées auraient besoin d'aide pour fonctionner ailleurs — un outil capable de
présenter la relying party d'origine à l'authentificateur. Vela livrait autrefois
une extension de navigateur de niveau développeur pour ce cas et l'a retirée en
septembre 2026 ; un chemin de récupération grand public pour la perte du domaine
reste un chantier ouvert, et nous le disons plutôt que de laisser croire qu'il
existe. L'accès on-chain indépendant dépend aussi de la prise en charge P-256
(RIP-7212) de la chaîne visée, qui s'améliore d'une chaîne à l'autre.

## Confidentialité

Pas de comptes, pas d'e-mail, pas de KYC, pas de phrase de récupération à
collecter. Les serveurs ne stockent que votre **clé publique** et un nom de compte
choisi (pour la récupération multi-appareils), publiés on-chain par conception. Le
contenu des transactions n'est pas journalisé. Le site utilise une analytique
auto-hébergée et sans cookies. Voir la
[politique de confidentialité](/privacy).

## Vérifiabilité et open source

Tout est **sous licence MIT et open source** — l'application et les quatre services
backend (données de chaîne, index de passkeys, relais, taux de change), que vous
pouvez **auto-héberger** (Réglages → Avancé → Points de terminaison des services).
Lisez le code sur
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

## Pas de token

Vela n'a **aucun token** et n'en prévoit pas. Il n'y a rien à acheter, à farmer ou
à spéculer. Le gas se paie dans l'actif natif de chaque réseau.

## État des audits et limites

Les **contrats Safe** au cœur de chaque compte Vela sont audités indépendamment et
éprouvés. L'**intégration propre à Vela** autour d'eux **n'a pas fait l'objet d'un
audit indépendant par un tiers**, et aucun n'est programmé aujourd'hui — un audit
professionnel est un objectif pour le moment où le projet pourra le financer, pas
un engagement daté. D'ici là, la revue de cette intégration est informelle : le
code est open source, et cela repose sur des membres compétents et intéressés de la
communauté qui le lisent, ainsi que sur une relecture assistée par IA. C'est utile,
mais ce n'est pas l'équivalent d'un audit professionnel. Traitez Vela comme un
logiciel en alpha et utilisez des montants que vous acceptez de confier à quelque
chose d'aussi jeune.

## Références

- ERC-4337 — abstraction de compte via l'EntryPoint
- EIP-1271 — norme de validation de signature pour les contrats
- ERC-7730 — signature lisible / descripteurs de données structurées
- EIP-5792 — regroupement d'appels de portefeuille
- RIP-7212 — précompilé pour la vérification de signature secp256r1 (P-256)
- WebAuthn / FIDO2 — authentification par passkey
- [Compte intelligent Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
