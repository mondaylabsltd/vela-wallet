---
title: Créer votre portefeuille
description: "Créer un portefeuille Vela avec une à sept clés — ce que fait chaque étape, pourquoi les clés sont fixées à la création, ce qui devient public, et ce qu'est réellement votre portefeuille."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Créer votre portefeuille

Créer un portefeuille prend une minute ou deux. Ouvrez le portefeuille web sur
[wallet.getvela.app](https://wallet.getvela.app/) — ou l'extension, l'app de bureau
ou l'app mobile — et choisissez **Créer un portefeuille**.

## Les étapes

1. **Nommez votre portefeuille.** Le nom vous aide à le reconnaître, et il est
   inscrit dans un registre public avec vos clés — considérez-le comme public et
   n'y mettez rien de personnel.
2. **Confirmez ce qui va se passer.** Vous cochez que vos clés publiques et le nom
   du portefeuille sont inscrits on-chain, que vos clés privées restent dans vos
   appareils ou vos clés de sécurité, et que vous acceptez les
   [conditions d'utilisation](/terms) et la
   [politique de confidentialité](/privacy).
3. **Créez votre première clé.** Choisissez comment : **cet appareil** (Face ID,
   Touch ID, empreinte, Windows Hello), **un téléphone ou une tablette** (scannez un
   QR code et créez-la sur cet appareil, quand l'app le propose) ou une **clé de
   sécurité USB**. Votre appareil crée la passkey puis signe une fois avec elle,
   pour que l'app sache que la clé fonctionne vraiment avant de continuer.
4. **Ajoutez d'autres clés, si vous le souhaitez.** Jusqu'à sept au total, de
   n'importe quel type. Chacune pourra signer seule. Si votre seule clé n'est
   synchronisée nulle part — une clé de sécurité, ou Windows Hello —, l'app en
   demande une deuxième : avec une seule clé non synchronisée, il suffit de perdre
   un appareil pour perdre le portefeuille.
5. **Créez.** L'app calcule l'adresse de votre portefeuille à partir de l'ensemble
   des clés et publie cet ensemble dans le registre public sur Gnosis Chain. Une
   fois cet enregistrement on-chain, votre portefeuille s'ouvre.

<Callout type="warning" title="Choisissez vos clés maintenant">
Votre adresse est calculée à partir des clés que vous avez au final : les clés ne
peuvent donc pas être ajoutées, retirées ou remplacées plus tard.
[Signataires et clés de sécurité](/fr/docs/signers) explique pourquoi, et comment
choisir.
</Callout>

## Ce qu'est votre portefeuille

Votre portefeuille est un **compte intelligent Safe** — un contrat, pas un simple
compte avec une seule clé privée. Vos clés en sont les propriétaires, et
n'importe laquelle peut autoriser une transaction. [Le contrat de compte](/fr/docs/account-contract)
liste tous les contrats en jeu.

L'adresse est **la même sur tous les réseaux**, et elle est **contrefactuelle** :
elle est calculée avant tout déploiement, si bien que vous pouvez recevoir des
fonds sur n'importe quel réseau tout de suite. Le contrat se déploie de lui-même la
première fois que vous envoyez depuis un réseau, et les frais de cette première
transaction incluent le déploiement. Créer le portefeuille ne vous coûte rien.

## Ce qui est public

<span id="what-is-public"></span>

Créer un portefeuille inscrit un enregistrement permanent dans un contrat de
registre public sur Gnosis Chain, lisible par tous et impossible à modifier ou à
supprimer :

- la **clé publique** de chaque clé (jamais la clé privée) et son **identifiant**
  (credential ID) ;
- le **modèle d'authentificateur** de chaque clé (quel gestionnaire de mots de passe
  ou quelle clé de sécurité l'a créée) et des indicateurs précisant si vous avez été
  vérifié et si la clé est synchronisée ;
- le **nom du portefeuille** et un **nom pour chaque clé** ;
- l'**adresse du portefeuille** et sa date de création ;
- les **données d'enregistrement signées** elles-mêmes.

L'index des clés publiques de Vela soumet l'enregistrement et en paie le gas : il le
voit donc en premier. Rien dans cet enregistrement ne peut déplacer vos fonds ; c'est
lui qui permet à n'importe laquelle de vos clés de retrouver le portefeuille sur un
nouvel appareil ([récupération](/fr/docs/recovery)). La
[politique de confidentialité](/privacy) donne la liste complète, et la
[page du registre](/registry) affiche tous les enregistrements.

## Prochaines étapes

- [Recevoir vos premiers jetons](/fr/docs/send-and-receive)
- [Comprendre les réseaux et les frais](/fr/docs/networks-and-fees)
- [Que faire si vous perdez un appareil](/fr/docs/recovery)
