---
title: Installer Vela
description: "Toutes les façons d'utiliser Vela — web, extension de navigateur, ordinateur et téléphone —, ce que chacune coûte, ce qu'elle permet, et ce dont votre appareil a besoin."
source: fa80f5cfdb95
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Installer Vela

Le même portefeuille fonctionne à plusieurs endroits, et tous ouvrent la même
adresse avec les mêmes clés. Choisissez selon vos besoins ; vous pouvez en utiliser
plusieurs. Les téléchargements sont sur [Obtenir Vela](/fr/get-started).

| | Ce que c'est | Prix | État |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) dans n'importe quel navigateur récent | Gratuit | En ligne |
| **Extension de navigateur** | Le portefeuille dans votre barre d'outils ; se connecte aux dApps | Gratuit | À télécharger et charger à la main ; pas encore sur le Chrome Web Store |
| **Bureau** | App native pour macOS, Windows et Linux | Gratuit | À télécharger depuis Obtenir Vela ou GitHub |
| **iPhone, Android** | Apps natives | Achat unique sur les stores | Pas encore sur les stores ; vous pouvez les compiler depuis les sources |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Ouvrir le portefeuille web →</a>

## Web

Rien à installer. Ouvrez [wallet.getvela.app](https://wallet.getvela.app/), créez un
portefeuille ou connectez-vous, et il est là. Votre liste de comptes est conservée
dans ce navigateur ; sur un autre appareil, il suffit de vous reconnecter avec
l'une de vos clés.

## Extension de navigateur

Navigateurs Chromium : Chrome, Edge et Brave (Chrome 116 ou plus récent). Elle
place le portefeuille dans la barre d'outils et permet aux dApps de s'y connecter
directement. Tant qu'elle n'est pas sur le Chrome Web Store :

1. Téléchargez l'extension depuis [Obtenir Vela](/fr/get-started) et décompressez-la
   dans un dossier que vous garderez — le navigateur l'exécute depuis ce dossier.
2. Ouvrez `chrome://extensions` et activez le **Mode développeur**.
3. Cliquez sur **Charger l'extension non empaquetée** et choisissez ce dossier.

C'est le même portefeuille : l'extension et le portefeuille web utilisent les mêmes
passkeys `getvela.app`, donc les mêmes clés ouvrent la même adresse.

## Bureau

Une vraie app native, pas une page web dans une fenêtre : **Windows** 10 et 11
(x64 et ARM), **macOS** 11 ou plus récent, et **Linux** (.deb, .rpm ou Flatpak,
x64 et ARM).

- **Windows** vous avertira qu'il « a protégé votre ordinateur », parce que
  l'installateur n'est pas encore signé. Choisissez **Informations
  complémentaires**, puis **Exécuter quand même**.
- Les versions **macOS** sont signées et notariées par Apple lors d'une étape
  séparée ; elles peuvent donc avoir du retard sur les autres plateformes. Quand le
  bouton Mac indique « Bientôt », la dernière version Mac notariée se trouve sur la
  page des versions GitHub.
- **Linux** : pour utiliser une clé de sécurité USB, votre système doit donner à
  l'app l'accès à cette clé — les paquets .deb et .rpm installent la règle pour vous.

Sur macOS et Windows, l'app de bureau intègre un navigateur pour les dApps. Les
sommes de contrôle de chaque paquet sont sur la
[page des versions GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) —
et vous pouvez vérifier plus qu'une somme de contrôle, voir plus bas.

## iPhone et Android

Des apps natives pour iOS 17.4 ou plus récent et Android 10 ou plus récent. Elles
seront vendues en achat unique sur l'App Store et Google Play ; elles ne sont
**pas encore sur les stores**. Le code est ouvert : vous pouvez donc les compiler
vous-même, gratuitement — à une différence près : une version que vous signez
vous-même ne peut pas utiliser les passkeys de votre propre téléphone pour les
portefeuilles getvela.app, mais le scan avec un autre téléphone et les clés de
sécurité USB fonctionnent. Voir [compiler les apps vous-même](/fr/docs/self-hosting#web-app).

## Vérifier ce que vous avez téléchargé

Une somme de contrôle vous dit que deux fichiers sont identiques. Elle ne peut pas
vous dire qui a fabriqué le fichier — et la liste des sommes de contrôle se trouve
sur la même page que le téléchargement. C'est pourquoi chaque paquet que nous
attachons à une version est aussi **attesté** : l'exécution du workflow qui l'a
compilé signe une déclaration nommant le fichier, le commit et cette exécution, et
GitHub la conserve. La vérifier tient en une commande avec la
[CLI GitHub](https://cli.github.com) (connectez-vous une fois avec `gh auth login` ;
la vérification est gratuite) :

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
```

Elle affiche qui a compilé le fichier et depuis quel commit, ou elle échoue. Rien sur
votre machine n'a besoin de nous faire confiance pour obtenir cette réponse : la
signature est celle de GitHub, produite au moment de la compilation, et quiconque se
contente de remettre un fichier en ligne quelque part ne peut pas la produire.

Les images Mac sont signées avec notre Developer ID et notariées par Apple, ce que
macOS vérifie pour vous à l'ouverture. Pour le lui demander vous-même :

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

<Callout type="warning" title="L'avertissement Windows reste">
L'attestation n'est pas une signature de code. L'installateur Windows n'est pas
signé : SmartScreen l'arrête donc encore une fois avec « Windows a protégé votre
ordinateur » — choisissez <strong>Informations complémentaires</strong>, puis
<strong>Exécuter quand même</strong>. Vérifier l'attestation, c'est le contrôle qui
vous dit que le fichier est bien le nôtre ; l'avertissement, lui, porte sur un
certificat que nous n'avons pas acheté.
</Callout>

Les paquets publiés avant la mise en place de ce mécanisme ne portent que leurs
sommes de contrôle.

## Utiliser Vela avec des dApps

<span id="dapps"></span>

Les dApps se connectent à Vela comme à n'importe quel portefeuille de navigateur
(EIP-1193 et EIP-6963) :

- dans un navigateur d'ordinateur, via l'**extension Vela pour navigateur** ;
- dans l'**app de bureau** (macOS, Windows), l'**app iPhone** et l'**app
  Android**, via leur navigateur intégré.

Le portefeuille web sur wallet.getvela.app ne se connecte pas aux dApps, et
WalletConnect n'est pas pris en charge. Chaque demande d'une dApp est décodée et
affichée avant que vous signiez — voir la [signature lisible](/fr/docs/clear-signing).

## Ce dont votre appareil a besoin

Vela signe avec des **passkeys**, que presque tous les appareils de ces dernières
années prennent en charge :

| Appareil | Pris en charge |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16+, macOS avec un Safari ou un Chrome récent |
| Android | Un Android récent avec les services Google Play, ou une clé de sécurité USB |
| Windows | Windows Hello avec Chrome ou Edge, ou une clé de sécurité |
| Linux | Une clé de sécurité, ou un téléphone à proximité (scannez le QR code) |

Si votre appareil ne peut pas conserver lui-même une passkey, utilisez un autre
téléphone ou une clé de sécurité matérielle.
[Signataires et clés de sécurité](/fr/docs/signers) indique quels types de clé
chaque app prend en charge.

## Les seules adresses officielles

- **getvela.app** — ce site, et les téléchargements
- **wallet.getvela.app** — le portefeuille web
- **github.com/mondaylabsltd** — le code et les paquets des versions

<Callout type="warning" title="Vérifiez avant d'installer">
Si quoi que ce soit vous envoie ailleurs pour « installer Vela » ou « vérifier
votre portefeuille », arrêtez-vous. Vela ne demande jamais de phrase de
récupération — il n'en a pas.
</Callout>

Ensuite : [créer votre portefeuille](/fr/docs/create-wallet).
