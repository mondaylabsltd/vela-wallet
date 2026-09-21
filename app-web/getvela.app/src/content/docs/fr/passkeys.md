---
title: Comment fonctionnent les passkeys
description: "Ce qu'est une passkey, où se trouve la clé privée selon le type de clé, pourquoi il n'y a aucun secret à hameçonner, et ce contre quoi une passkey ne vous protège pas."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Comment fonctionnent les passkeys

Les clés qui contrôlent un portefeuille Vela sont des **passkeys** : des
identifiants WebAuthn sur la courbe P-256. Votre appareil ou votre clé de sécurité
crée chacune d'elles, garde la clé privée, et ne l'utilise qu'après votre
confirmation par Face ID, une empreinte, le code de votre appareil, ou un appui et
un code PIN sur une clé de sécurité. Vela ne reçoit jamais la clé privée ; si un
gestionnaire de mots de passe la synchronise, c'est lui qui la conserve, chiffrée,
pour votre compte.

## Ce qu'est une passkey

Une passkey est une paire de clés publique/privée créée pour un seul site web —
pour Vela, `getvela.app`. Une app ne reçoit jamais la clé privée ; elle peut
seulement demander à l'authentificateur de signer quelque chose, et
l'authentificateur vous demande d'abord votre accord.

L'endroit où se trouve la clé privée dépend du type de clé :

| Type de clé | Où se trouve la clé privée | Synchronisée sur d'autres appareils ? |
| --- | --- | --- |
| **Cet appareil** — Face ID, Touch ID, empreinte, Windows Hello | Le gestionnaire de mots de passe de votre plateforme (trousseau iCloud, gestionnaire de mots de passe de Google) ou un gestionnaire comme 1Password | En général oui, chiffrée de bout en bout, si la synchronisation est activée. Les clés Windows Hello restent sur le PC |
| **Un autre téléphone**, utilisé en scannant un QR code | Le gestionnaire de mots de passe de ce téléphone | Comme ci-dessus |
| **Une clé de sécurité matérielle** (YubiKey et autres clés FIDO2, en USB ou NFC) | Dans la clé de sécurité | Jamais |

Un portefeuille Vela peut utiliser jusqu'à sept clés, dans n'importe quelle
combinaison, choisies à sa création ;
[signataires et clés de sécurité](/fr/docs/signers) traite de ce choix.

## Aucun secret à hameçonner

L'hameçonnage consiste à vous faire livrer un secret. Une phrase de récupération,
ce sont douze mots que l'on peut vous convaincre de taper quelque part. Une passkey
n'a **aucun secret que vous puissiez taper** : rien à révéler, rien à coller, et un
faux site ne peut pas vous la demander. Et comme une passkey est créée pour un seul
site web, votre navigateur ne propose une passkey `getvela.app` qu'aux pages de
getvela.app et de ses sous-domaines.

Cela élimine toute une catégorie de pertes — la phrase de récupération volée —,
fréquente en auto-conservation.

## Ce contre quoi une passkey ne vous protège pas

<Callout type="warning" title="Une passkey signe tout ce que vous approuvez">
L'invite de votre téléphone ou de votre navigateur indique <em>quelle</em> clé est
utilisée, pas <em>ce qui</em> est signé. Une passkey signera une transaction
malveillante aussi volontiers qu'une transaction légitime si vous l'approuvez.
C'est pourquoi Vela décode chaque transaction avant que vous signiez
(<a href="/fr/docs/clear-signing">signature lisible</a>), et pourquoi la page qui
l'affiche compte (<a href="/fr/docs/bybit-attack">l'attaque de Bybit</a>).
</Callout>

Elle ne protège pas non plus contre quelqu'un qui a votre téléphone déverrouillé et
peut passer sa vérification, ni contre quelqu'un qui contrôle le compte par lequel
votre passkey se synchronise. Gardez un code de verrouillage sur votre appareil,
sécurisez votre compte Apple ou Google, et envisagez une clé de sécurité matérielle
qui ne se synchronise nulle part.

## À quoi ressemble une signature

1. Vous confirmez une transaction dans Vela, après avoir lu ce qu'elle fait.
2. Votre appareil ou votre clé de sécurité vous demande Face ID, une empreinte,
   votre code PIN, ou un appui et un code PIN.
3. Il signe, et seule la signature revient à l'app.
4. L'app transmet l'opération signée au relais, qui la soumet ; le contrat de votre
   portefeuille vérifie la signature de la passkey on-chain avant de faire quoi que
   ce soit.

## Où va la clé publique

La partie **publique** de vos clés est enregistrée dans un registre public sur
Gnosis Chain, pour qu'un nouvel appareil puisse retrouver votre portefeuille. C'est
le sujet de [récupération et connexion](/fr/docs/recovery).

Ensuite : [signataires et clés de sécurité](/fr/docs/signers).
