---
title: Envoyer et recevoir
description: Comment recevoir et envoyer des tokens dans Vela — une seule adresse sur tous les réseaux, des transactions signées lisiblement, et comment l'abstraction de compte déplace réellement vos fonds.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Envoyer et recevoir

## Recevoir

1. Ouvrez votre portefeuille et appuyez sur **Recevoir**.
2. Partagez votre adresse — copiez-la, ou laissez l'expéditeur scanner le QR code.
3. Quand le transfert est confirmé on-chain, le solde apparaît dans votre
   portefeuille.

Deux choses valent d'être sues :

- Votre adresse est **la même sur tous les réseaux pris en charge**, vous ne
  partagez donc qu'une seule adresse — assurez-vous simplement que l'expéditeur
  utilise le bon réseau.
- Vous pouvez **recevoir avant que votre portefeuille soit déployé**. Les comptes
  Vela sont des comptes intelligents contrefactuels : les fonds peuvent arriver à
  votre adresse avant que le contrat existe sur une chaîne donnée ; il se déploie
  tout seul lors de votre premier envoi.

## Envoyer

1. Appuyez sur **Envoyer** et choisissez le **token**.
2. Saisissez le **montant** (vous pouvez basculer entre le token et votre devise
   d'affichage) et le **destinataire**. Quand il le peut, Vela résout les
   destinataires connus en un nom — un compte Vela, un nom ENS, un Basename, etc.
3. **Vérifiez et confirmez.** Vela affiche le transfert, puis demande votre
   passkey (Face ID / Touch ID / empreinte).

### Ce qui se passe quand vous confirmez

Vela ne se contente pas de « diffuser » une transaction. Sous le capot :

1. Il construit une **UserOperation** ERC-4337 pour votre compte Safe.
2. Votre appareil la signe avec une assertion **WebAuthn (P-256)** après votre
   contrôle biométrique.
3. L'opération signée part vers le **relais**, qui la soumet à l'EntryPoint ;
   votre Safe vérifie la signature P-256 **on-chain** puis exécute.

<Callout type="info" title="Le relais ne peut pas altérer votre transaction">
Le relais reçoit une UserOperation <strong>déjà signée</strong>. Il peut retarder
ou refuser de relayer, mais il ne peut changer ni le destinataire, ni le montant,
ni aucun autre champ — toute modification invalide votre signature. C'est une aide
à la disponibilité, pas un dépositaire, et c'est open source : vous pouvez faire
tourner le vôtre.
</Callout>

### Signature lisible — aucune approbation à l'aveugle

Avant que vous signiez, Vela décode la transaction à l'aide des descripteurs
**ERC-7730** et affiche l'**intention** (Envoyer, Approuver, Échanger…), les
**montants et adresses**, et une indication de risque — pas de l'hexadécimal
opaque. Quand il ne peut pas décoder complètement un appel, il affiche un
**avertissement explicite de signature à l'aveugle** au lieu de faire semblant de
comprendre. Une approbation de token illimitée n'est pas seulement signalée : Vela
la réécrit en un montant fini et refuse de soumettre une approbation qui resterait
illimitée.

## Avant d'appuyer sur envoyer

- **Vérifiez les premiers et derniers caractères de l'adresse.** Les logiciels
  malveillants qui remplacent les adresses existent vraiment.
- **Confirmez le réseau.** Envoyer sur le mauvais réseau est l'erreur coûteuse la
  plus fréquente. Voir [Réseaux et frais](/fr/docs/networks-and-fees).
- **Commencez petit avec un nouveau destinataire.** Un minuscule transfert de test
  est une assurance bon marché.

Les transactions sont irréversibles. Aucun service client ne peut récupérer un
envoi à la mauvaise adresse — c'est la nature de l'auto-conservation.

## Lire votre historique

Les soldes et l'historique sont lus en direct depuis un pool de points de
terminaison RPC publics, avec bascule automatique. Si le réseau est lent,
l'historique peut prendre un instant — un indicateur qui tourne veut dire « on
récupère encore », pas « les fonds ont disparu ».
