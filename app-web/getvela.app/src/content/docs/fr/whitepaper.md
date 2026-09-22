---
title: Livre blanc
description: "Comment fonctionne Vela, et ce à quoi vous devez — ou non — faire confiance pour l'utiliser : le compte, les clés, les frais, le modèle de menaces, la récupération, et ce qui se passe si Vela disparaît."
source: 662b69510225
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Livre blanc

<Callout type="info" title="État : alpha · dernière révision en septembre 2026">
Cette page décrit comment Vela fonctionne aujourd'hui, et ce à quoi vous devez ou
non faire confiance pour l'utiliser. Vela est en <a href="/blog/vela-is-in-alpha">alpha</a>
— commencez avec de petits montants. Vela n'a pas de jeton. Tout ce qui figure ici
peut être vérifié dans le code open source ; là où le code et cette page divergent,
c'est le code qui a raison, et cette page qui comporte un bug.
</Callout>

## Résumé

Vela est un **portefeuille à contrat intelligent en auto-conservation** pour
Ethereum et les autres réseaux EVM. Chaque portefeuille est un compte **Safe
v1.4.1** non modifié, actionné via **ERC-4337** et contrôlé par jusqu'à sept
**passkeys** — des clés WebAuthn P-256 conservées par vos appareils, votre
gestionnaire de mots de passe ou des clés de sécurité matérielles. Il n'y a pas de
phrase de récupération.

L'entreprise Vela ne détient jamais vos clés et n'a aucun rôle sur votre Safe ;
elle **ne peut donc ni déplacer, ni geler, ni saisir vos fonds** d'elle-même. Elle
écrit et distribue en revanche le logiciel qui demande à vos clés de signer — c'est
pourquoi le modèle de menaces ci-dessous compte. Les apps, le relais qui soumet les
transactions et les services annexes sont open source, et vous pouvez faire tourner
votre propre copie de chacun. Ce à quoi vous faites confiance, en bref : les
contrats, les authentificateurs qui détiennent vos clés, le code de l'app avec
laquelle vous signez, le domaine auquel appartiennent vos passkeys, et les
services vers lesquels vous faites pointer l'app.

## Pourquoi Vela existe

- Les **portefeuilles à phrase de récupération** placent un secret de 12 à 24 mots
  devant chaque utilisateur : un point de défaillance unique, et une cible permanente
  pour l'hameçonnage.
- Les **portefeuilles dépositaires** suppriment la phrase de récupération en prenant
  la garde des fonds.
- Les **portefeuilles à passkey** qui dépendent des serveurs et du code fermé d'une
  seule entreprise suppriment la phrase de récupération, mais vous laissent en plan
  si l'entreprise disparaît.
- La **signature à l'aveugle** — approuver des données opaques que vous ne pouvez
  pas lire — reste courante, et c'est l'une des façons dont les portefeuilles se font
  vider.

Vela vise le confort d'une passkey sans aucune de ces dépendances : un compte
standard, du code ouvert, des services remplaçables, et des transactions que vous
pouvez lire avant de signer.

## Principes de conception

1. **L'auto-conservation, sans exception.** Les clés sont créées et conservées par
   vos authentificateurs. Les services de Vela ne les voient jamais ; ce qu'ils
   voient est listé dans la section Confidentialité.
2. **Des contrats standards, non modifiés.** Aucun contrat sur le chemin de vos
   fonds n'a été écrit par Vela.
3. **Vérifier plutôt que faire confiance.** Les apps et les services sont publics ;
   les services peuvent être auto-hébergés.
4. **Décoder avant de signer.** Ce qui ne peut pas être décodé est accompagné d'un
   avertissement explicite de signature à l'aveugle.
5. **En faire moins.** Le portefeuille envoie, reçoit et signe pour les dApps que
   vous choisissez.

## Architecture

```text
Apps Vela — web, extension de navigateur, bureau (macOS/Windows/Linux), iOS, Android
  un cœur Rust partagé (règles, crypto, ABI, signature lisible) + une interface native par plateforme
  • construit la UserOperation et montre ce qu'elle fait
  • demande à votre clé une assertion WebAuthn
        │  UserOperation signée (frais inclus)
        ▼
Relais (vela-relay, auto-hébergeable)
  • chiffre les frais, avance le gas, soumet handleOps
  • ne peut pas modifier l'opération
        ▼
Chaîne EVM
  EntryPoint v0.7 → votre Safe v1.4.1 → module 4337 de Safe
  le module passkey de Safe vérifie la signature P-256 via le précompilé RIP-7212
```

Services annexes, tous open source : un **index des clés publiques** qui enregistre
les nouveaux portefeuilles dans un registre on-chain et répond aux recherches, un
annuaire de **données de chaîne**, et un flux de **taux de change**. Voir le
[guide d'auto-hébergement](/fr/docs/self-hosting).

### Le compte

Votre portefeuille est un proxy **Safe v1.4.1** (singleton SafeL2), avec le
**module 4337 v0.3.0** de Safe activé comme module et comme fallback handler,
actionné via l'**EntryPoint v0.7**. Ses propriétaires sont des signataires passkey
issus du **module passkey v0.2.1** de Safe : la première clé est vérifiée par le
signataire partagé, et chaque clé supplémentaire par son propre contrat signataire,
créé par la fabrique de Safe. Le seuil est de **1**.

L'adresse est **déterministe et contrefactuelle** : elle est calculée avec
`CREATE2` à partir des données de configuration du Safe, qui incluent chaque clé
fondatrice, avant tout déploiement. Elle est la même sur tous les réseaux. Vous
pouvez y recevoir des fonds immédiatement ; votre première transaction sur chaque
réseau déploie le portefeuille et en paie le coût dans les frais de cette
transaction.

### Les clés

Un portefeuille a **d'une à sept clés**, fixées à sa création. N'importe laquelle
peut signer seule (1-of-n). Une clé peut être :

- une passkey sur l'appareil que vous utilisez — synchronisée par le trousseau
  iCloud, le gestionnaire de mots de passe de Google ou un autre gestionnaire de
  mots de passe, si vous l'autorisez ;
- un autre téléphone, utilisé en scannant un QR code (le transport hybride de
  WebAuthn) ;
- une clé de sécurité matérielle en USB ou NFC, qui ne se synchronise nulle part.

Chaque signature exige la vérification de l'utilisateur propre à
l'authentificateur — biométrie ou code de l'appareil, ou code PIN et appui sur une
clé de sécurité. Il n'y a pas de clé de session. Les clés ne peuvent pas être
ajoutées, retirées ou remplacées plus tard : sur chaque chaîne où le portefeuille
n'est pas encore déployé, l'adresse correspond toujours à l'ensemble fondateur, si
bien que changer de propriétaires sur une chaîne rendrait le compte différent d'une
chaîne à l'autre.

Les passkeys appartiennent à une partie de confiance (relying party) — celles de
Vela sont créées pour **`getvela.app`**. Les navigateurs ne les proposent qu'aux
pages de getvela.app ou de ses sous-domaines, ce qui les rend résistantes à
l'hameçonnage ; c'est aussi une dépendance, sur laquelle ce document revient plus
bas.

### Le parcours de signature

1. **Construire** une UserOperation pour votre Safe — incluant un transfert qui
   paie le relais — et la simuler.
2. **Décoder** cette opération en intention lisible et vous la montrer.
3. **Signer** : votre authentificateur produit une assertion WebAuthn sur le hash
   de l'opération après vous avoir vérifié.
4. **Encoder** l'assertion dans le format de signature Safe qu'attend le module
   passkey.
5. **Soumettre** l'opération signée au relais, qui appelle l'EntryPoint.
6. **Vérifier on-chain** : le module passkey contrôle la signature P-256 avec le
   précompilé RIP-7212 avant que le Safe n'exécute quoi que ce soit. Il n'y a pas de
   vérificateur de repli ; un réseau sans le précompilé ne peut pas être ajouté.

### Les frais

- Le relais est payé **au sein de l'opération** : l'opération déclare des frais
  EntryPoint nuls et inclut un transfert de votre Safe vers l'adresse du relais. Le
  montant et le destinataire font partie de ce que vous signez : vous payez donc
  exactement ce qu'affichait l'écran de confirmation.
- Les frais valent **trois fois le gas que le portefeuille réserve pour
  l'opération** (les estimations simulées majorées de moitié, avec des minimums),
  **au plus élevé du prix du gas relevé par le portefeuille lui-même et du prix du
  relais pour la vitesse choisie**, avec un minimum d'environ 0,01 $. Sur Tempo, le
  multiplicateur est de deux. La marge sur la réserve et sur le prix place les frais
  au-dessus du coût réel de l'opération on-chain, et plus encore pour la première
  transaction sur un réseau ; le relais garde la différence. Le montant exact figure
  sur l'écran de confirmation avant que vous signiez.
- Les frais vont au relais configuré dans le portefeuille : celui de Vela par
  défaut, ou n'importe quel déploiement de vela-relay, y compris celui que vous
  faites tourner.
- Les frais se paient dans la monnaie du réseau ou dans un stablecoin en dollars que
  le relais accepte (du pathUSD sur Tempo, qui n'a pas de monnaie native). Il n'y a
  **pas de paymaster** : personne ne sponsorise le gas, et personne ne peut filtrer
  les transactions au moyen d'une politique de sponsoring.
- Si la trésorerie de gas propre à un relais est vide sur un réseau, le
  portefeuille vous le dit avant que vous signiez. Il n'y a pas de dépôt par
  utilisateur.

Détails : [réseaux et frais](/fr/docs/networks-and-fees).

### La signature lisible

Les appels et les messages EIP-712 sont décodés avec des descripteurs **ERC-7730**
— intégrés à l'app pour les contrats courants, récupérés sur le service de données
de chaîne, ou rapprochés de formes standards de jetons —, puis, en dernier recours,
à l'aide d'une base publique de sélecteurs, avec la mention « au mieux ». Tout ce
qui reste reçoit un avertissement explicite de signature à l'aveugle. Les
descripteurs récupérés ne sont pas authentifiés cryptographiquement. Une
approbation on-chain au niveau « illimité » (2^200 ou plus) ne peut pas être soumise
tant que vous ne l'avez pas réduite ; une approbation finie mais élevée et les
permits signés s'affichent avec un avertissement, sans être bloqués. Détails :
[signature lisible](/fr/docs/clear-signing).

### Les réseaux

Vela intègre 24 réseaux — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable,
Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume et XRPL EVM — et
accepte tout réseau EVM qui dispose des onze contrats qu'il vérifie et du
précompilé RIP-7212. (Les clés deux à sept ont aussi besoin de la fabrique de
signataires passkey de Safe sur ce réseau, ce que la vérification ne couvre pas
encore.)

## Modèle de sécurité

**Ce que Vela ne peut pas faire**

- Déplacer, dépenser ou geler vos fonds de lui-même — seules vos clés autorisent
  votre Safe, et Vela n'y a aucun rôle. (Ce que Vela peut faire, c'est distribuer
  un logiciel qui vous demande de signer ; voir les menaces ci-dessous.)
- Modifier une transaction après que vous l'avez signée — toute modification
  invalide la signature.
- Lire vos clés privées — elles restent dans vos authentificateurs.
- Ajouter une clé à votre portefeuille, ou en retirer une.

**Ce que « ne peut pas geler » ne couvre pas : le jeton lui-même.** L'USDC, l'USDT
et la plupart des jetons adossés à une monnaie fiduciaire permettent à leur
émetteur de mettre n'importe quelle adresse sur liste noire, y compris la vôtre. Ce
pouvoir appartient à l'émetteur et existe quel que soit le portefeuille que vous
utilisez. Ce que l'auto-conservation vous apporte, c'est que Vela n'est pas une
seconde partie capable de le faire.

**Ce à quoi vous faites confiance**

- Les **contrats** : Safe, ses modules 4337 et passkey, l'EntryPoint v0.7, et le
  précompilé RIP-7212 de la chaîne.
- Le **domaine** : toute page servie depuis getvela.app ou l'un de ses
  sous-domaines peut demander une signature à vos clés.
- Les **authentificateurs** qui détiennent vos clés et — pour les passkeys
  synchronisées — le compte Apple, Google ou de gestionnaire de mots de passe qui
  se trouve derrière.
- **Le code de l'app avec laquelle vous signez.** C'est elle qui construit la
  transaction et vous montre ce qu'elle fait. Une app compromise peut vous montrer
  une chose et vous faire signer autre chose ; l'invite de l'authentificateur ne
  vous signalera pas la différence.
- Les **points d'accès RPC** que vous lisez : un nœud menteur peut afficher de faux
  soldes ou un faux aperçu de simulation. Vous pouvez définir les vôtres.
- Les **services de données de chaîne et de taux de change** : ils fournissent les
  listes de jetons, les descripteurs, la liste des jetons acceptés pour les frais et
  les taux utilisés pour convertir un montant en monnaie fiduciaire en montant de
  jetons.
- Le **relais** : il ne peut pas modifier ce que vous avez signé, mais il peut le
  retarder ou le refuser, choisir le moment de son inclusion (il pourrait donc
  devancer un swap dans la limite de votre slippage), et fixer le prix du gas sur
  lequel vos frais sont calculés, jusqu'à trois fois le relevé du portefeuille
  lui-même.

**Menaces prises en compte**

- **Appareil perdu ou volé** — un voleur doit encore passer la vérification de
  l'authentificateur ; une autre clé vous rend l'accès. Mais une clé ne peut pas être
  retirée : si l'une d'elles est peut-être entre les mains de quelqu'un d'autre,
  transférez vos fonds vers un nouveau portefeuille, car l'ancienne adresse reste
  utilisable par cette clé sur tous les réseaux.
- **Hameçonnage** — une passkey ne peut pas être tapée sur un faux site, et les
  navigateurs ne la proposent qu'aux pages de getvela.app et de ses sous-domaines.
- **dApp malveillante** — traitée par la signature lisible et le garde-fou sur les
  approbations, avec une lacune sérieuse : une dApp peut demander un appel de votre
  Safe vers lui-même — `enableModule`, `addOwnerWithThreshold`,
  `setFallbackHandler`, `setGuard` —, et n'importe lequel de ces appels, signé une
  seule fois, livre le compte aussi complètement que la charge utile de Bybit. Vela
  décode ces appels mais ne les bloque pas encore. Refusez toute demande dont la
  cible est l'adresse de votre propre portefeuille.
- **Service backend compromis** (relais, index, données de chaîne, taux de change)
  — aucun pouvoir de signature, mais une influence réelle : refus de service,
  descripteurs ou listes de jetons trompeurs, taux de change faux qui modifient ce
  qu'un montant en monnaie fiduciaire envoie réellement, et (pour le relais) le
  moment d'inclusion et le prix du gas évoqués plus haut. Les descripteurs
  récupérés ne sont pas considérés comme authentifiés, et chaque service peut être
  remplacé.
- **Distribution de l'app compromise** — un déploiement web, une mise à jour de
  l'extension ou une version d'app altérés pourraient vous présenter une transaction
  malveillante à signer. C'est la classe d'attaque de [Bybit](/fr/docs/bybit-attack).
  Les parades actuelles sont limitées : le décodage et le garde-fou sur les
  approbations dans l'app elle-même, des versions macOS notariées, et la compilation
  de l'extension ou des apps depuis les sources par vous-même (les paquets publiés
  sont accompagnés de sommes de contrôle SHA-256, pas de signatures). Une page de
  signature indépendante, qui ne partage pas le code de l'app, est construite mais
  pas encore reliée.
- **Tout ce qui est servi depuis le domaine** — n'importe quelle page de
  getvela.app ou de ses sous-domaines, y compris un script qu'elle charge, pourrait
  demander des signatures aux passkeys Vela, et l'invite n'affiche que
  « getvela.app ». Le site web interdit donc à ses propres pages d'utiliser les
  passkeys, et tient son script d'analyse d'audience à l'écart de la page qui
  détient une clé. Si le domaine changeait de mains, son nouveau propriétaire
  contrôlerait aussi les apps autorisées à utiliser les passkeys. L'extension et les
  apps compilées vous-même embarquent leur propre code, même si, par défaut, elles
  récupèrent encore les descripteurs et utilisent des services hébergés sous
  getvela.app.

## Récupération

Créer un portefeuille publie ses clés publiques et son adresse dans un **contrat de
registre** public sur Gnosis (qui peut être copié sur Ethereum). Sur un nouvel
appareil, vous vous connectez avec **n'importe laquelle** de vos clés ; l'app
retrouve le portefeuille via l'index ou, à défaut, directement dans le registre, et
vérifie que les clés aboutissent bien à l'adresse enregistrée. Un portefeuille à
clé unique peut aussi être reconstruit à partir de deux signatures, sans aucun
registre.

<Callout type="warning" title="Vos clés sont votre récupération">
Il n'y a ni phrase de récupération, ni récupération sociale, ni gardien — rien que
Vela puisse perdre, divulguer ou être contraint d'utiliser. Si toutes les clés
fondatrices sont perdues, le portefeuille ne peut pas être récupéré. Créez le
portefeuille avec plus d'une clé, laissez la synchronisation des passkeys activée
si vous comptez dessus, et sécurisez le compte qui se trouve derrière.
</Callout>

Détails : [récupération et connexion](/fr/docs/recovery).

## Si Vela disparaît

Vos fonds restent dans votre Safe, on-chain. Les contrats ne dépendent pas de Vela,
et chaque service que Vela fait tourner est open source, donc exploitable par
quelqu'un d'autre. La seule chose qui ne peut pas changer de place, c'est la
partie de confiance des passkeys, `getvela.app` : une copie du portefeuille web
sur un autre domaine crée un autre portefeuille. Pour les
portefeuilles existants, l'extension Vela pour navigateur (qui peut utiliser les
passkeys `getvela.app` sur autorisation) et les apps que vous compilez vous-même
(avec un téléphone ou une clé de sécurité) continuent de fonctionner sans
getvela.app. Le [guide d'auto-hébergement](/fr/docs/self-hosting#if-getvela-app-disappears)
détaille chaque solution et ses limites. Accéder de façon indépendante à une
chaîne exige aussi que cette chaîne prenne en charge RIP-7212.

## Confidentialité

Ni compte, ni e-mail, ni KYC. Ce qui devient public est inscrit dans le registre
quand vous créez un portefeuille : la clé publique et l'identifiant (credential ID)
de chaque clé, le modèle d'authentificateur, le nom de votre portefeuille et les
noms de vos clés, l'adresse, et les données d'enregistrement signées. L'index de
Vela voit cet enregistrement avant de le soumettre, ainsi que les adresses dont
vous recherchez le nom ; le relais de Vela voit votre adresse, les opérations que
vous soumettez et le point d'accès RPC utilisé par votre app (y compris toute clé
d'API présente dans son URL), et conserve les opérations pendant une durée limitée
pour les relancer et diagnostiquer les problèmes. Chaque service voit votre adresse
IP. Le site web utilise un outil d'analyse d'audience sans cookies. La
[politique de confidentialité](/privacy) est la liste qui fait foi.

## Open source

Le portefeuille (toutes les apps et le cœur), le relais et le service de taux de
change sont sous licence MIT ; l'annuaire de données de chaîne l'est aussi.
L'index des clés publiques est public, mais n'a pas encore de fichier de licence.
Le code :
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Aucun jeton

Vela n'a pas de jeton et n'en prévoit pas. Il n'y a rien à acheter, à farmer ni sur
quoi spéculer. Les frais se paient dans la monnaie de chaque réseau ou en
stablecoin.

## État des audits et limites

Les contrats de Safe, ses modules 4337 et passkey, et l'EntryPoint v0.7 ont été
audités de façon indépendante et sont largement utilisés. **Le code de Vela — les
apps, les services backend et le contrat de registre — n'a fait l'objet d'aucun
audit tiers indépendant, et aucun n'est programmé** ; un audit professionnel est un
objectif pour le jour où le projet pourra en financer un, pas un engagement daté.
D'ici là, la revue est informelle : le code est ouvert, des membres compétents de la
communauté le lisent, et il est revu à l'aide d'outils d'IA. Cela aide ; ce n'est pas
l'équivalent d'un audit professionnel. Considérez Vela comme un logiciel en alpha.
Détails : [audits et problèmes connus](/fr/docs/security-audits).

## Références

- ERC-4337 — Abstraction de compte via l'EntryPoint
- EIP-1271 — Validation de signature pour les contrats
- ERC-7730 — Descripteurs de signature lisible
- EIP-5792 — Regroupement d'appels par le portefeuille (`wallet_sendCalls`)
- RIP-7212 / EIP-7951 — Précompilé de vérification de signature P-256
- WebAuthn / FIDO2 — Passkeys
- [Compte intelligent Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
