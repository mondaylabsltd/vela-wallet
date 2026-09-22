---
title: Envoyer et recevoir
description: "Recevoir et envoyer avec Vela — une seule adresse sur tous les réseaux, envoyer à une ou plusieurs personnes, d'où viennent les noms des destinataires, ce que vous confirmez, et comment le relais transmet vos fonds."
source: c23b205bcd8b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Envoyer et recevoir

## Recevoir

1. Ouvrez votre portefeuille et touchez **Recevoir**.
2. Partagez votre adresse — copiez-la ou affichez le QR code. Le portefeuille web
   peut aussi créer une demande de paiement qui inclut un montant.
3. Une fois le transfert confirmé on-chain, il apparaît dans votre solde.

- Votre adresse est **la même sur tous les réseaux** : vous ne donnez qu'une
  seule adresse — mais l'expéditeur doit tout de même utiliser un réseau que Vela
  prend en charge, ou que vous avez ajouté.
- Vous pouvez **recevoir avant que votre portefeuille soit déployé** sur un
  réseau. Il se déploie de lui-même lors de votre premier envoi sur ce réseau.

## Envoyer

1. Touchez **Envoyer** et choisissez le **jeton**.
2. Saisissez le **montant** (en jeton ou dans votre devise d'affichage) et
   l'**adresse du destinataire**, en la collant, en scannant un QR code ou en
   choisissant un contact.
3. **Vérifiez.** Vela affiche ce qui va se passer, les frais, et le nom trouvé pour
   le destinataire, s'il y en a un.
4. **Confirmez** avec l'une de vos clés — Face ID, une empreinte, un code PIN, ou
   un appui et un code PIN sur votre clé de sécurité.

### Envoyer à plusieurs personnes, ou tout regrouper

- **Répartition** — envoyez un même jeton à plusieurs personnes en une seule
  transaction. Vous pouvez coller une liste ou importer un tableur, et saisir les
  montants dans votre devise.
- **Regroupement** — envoyez plusieurs jetons vers une seule adresse en une seule
  transaction.

Dans les deux cas, vous signez une seule fois, et la transaction ne paie qu'une
seule fois des frais.

### Les noms des adresses

Quand vous saisissez une adresse, Vela lui cherche un nom : d'abord dans son propre
registre (le nom d'un autre portefeuille Vela), puis dans les enregistrements
inverses `.bnb`, `.arb`, `.g`, Basename et ENS, lus directement sur chaque chaîne.
Cela ne fonctionne que dans un sens — Vela nomme une adresse que vous avez saisie.
Taper un nom comme `alice.eth` ne permet pas de trouver une adresse. Vos
**contacts** enregistrés affichent aussi leur nom. Voyez un nom comme un indice,
pas comme une preuve : un enregistrement inverse ou un nom de portefeuille Vela est
choisi par celui qui contrôle cette adresse.

### Monnaie des frais et vitesse

L'écran de confirmation affiche les frais dans la monnaie de paiement choisie et
dans votre devise. Vous pouvez payer avec la monnaie native du réseau ou, là où le
relais l'accepte, avec un stablecoin en dollars, et choisir une vitesse (par
défaut : rapide). Quand vous envoyez le **maximum** d'une monnaie native, Vela
garde de côté de quoi payer les frais. [Comment les frais sont calculés](/fr/docs/networks-and-fees).

### Ce qui se passe quand vous confirmez

1. Vela construit une **UserOperation** ERC-4337 pour votre Safe, qui inclut le
   paiement des frais au relais.
2. Votre clé la signe avec une assertion **WebAuthn (P-256)** après vous avoir
   vérifié.
3. L'opération signée part vers le **relais**, qui la soumet à l'EntryPoint ; votre
   Safe vérifie la signature P-256 on-chain, puis exécute.

<Callout type="info" title="Le relais ne peut pas modifier votre transaction">
Le relais reçoit une opération déjà signée. Il ne peut modifier ni le
destinataire, ni le montant, ni les frais — toute modification invalide votre
signature. Il peut la retarder ou la refuser, et c'est lui qui décide du moment où
elle est incluse. Il est open source, et vous pouvez
[faire tourner le vôtre](/fr/docs/self-hosting#relay).
</Callout>

Avant que vous signiez, Vela décode ce que fait la transaction et vous avertit de ce
qu'il ne parvient pas à décoder ; voir la [signature lisible](/fr/docs/clear-signing).

## Avant d'appuyer sur Envoyer

- **Vérifiez le début et la fin de l'adresse.** Les logiciels malveillants qui
  remplacent les adresses existent bel et bien, tout comme les adresses
  ressemblantes glissées dans votre historique.
- **Vérifiez le réseau.** Envoyer sur le mauvais réseau est une erreur courante, et
  coûteuse.
- **Commencez petit avec un nouveau destinataire.** Un minuscule transfert de test
  est une assurance bon marché.

Les transactions sont irréversibles. Personne ne peut récupérer un envoi à la
mauvaise adresse — c'est la nature même de l'auto-conservation.

## Votre activité

Votre activité combine ce que vous avez envoyé depuis cet appareil et les
transferts de jetons lus dans les logs de chaque chaîne. Sur certains réseaux, un
simple transfert de monnaie native qui vous arrive via un autre contrat (certains
retraits de plateformes d'échange, par exemple) peut ne produire aucun log : il
peut alors apparaître dans votre solde sans figurer dans votre activité. Les soldes
sont lus en direct via un ensemble de points d'accès RPC avec bascule automatique ;
un indicateur de chargement signifie « encore en cours de lecture », pas « fonds
disparus ».

Ensuite : [réseaux et frais](/fr/docs/networks-and-fees).
