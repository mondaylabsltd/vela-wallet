---
title: Pourquoi nous avons créé Vela
description: "La version longue — où êtes-vous censé garder douze mots, ce que les passkeys ont changé, ce que nous ne pouvions pas accepter dans les portefeuilles que nous utilisions déjà, et le compromis que nous avons choisi à la place."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Pourquoi nous avons créé Vela

Nous ne voulions pas faire un portefeuille de plus. Tout est parti d'une question
à laquelle nous n'avons jamais su répondre proprement :

> Où êtes-vous censé garder douze mots ?

## La réponse honnête, c'est une capture d'écran

Mettez-les dans vos notes, et il suffit d'un téléphone volé pour avoir des ennuis.
Écrivez-les sur papier, et vous voilà à penser au feu, à l'eau, aux déménagements,
aux colocataires, aux sacs-poubelle, et à vous demander si, dans quelques années,
vous vous souviendrez encore de « l'endroit sûr ».

Pour beaucoup, la réponse honnête est une capture d'écran dans la pellicule. Tout
le monde sait que c'est une mauvaise idée. Tout le monde le fait quand même —
parce que la « bonne » réponse est trop lourde à vivre au quotidien.

Une phrase de récupération, c'est un secret qui doit survivre à des décennies de
vie ordinaire sans jamais être copié, photographié, tapé dans le mauvais champ ou
lu à voix haute à quelqu'un de serviable au téléphone. Ce n'est pas un problème
difficile pour une personne très prudente. C'est un problème difficile pour une
personne.

## Puis les passkeys ont changé ce qu'un portefeuille peut être

Nous utilisions [Base Account](https://account.base.app) tous les jours, et signer
avec Face ID paraissait évident, comme jamais les phrases de récupération ne
l'avaient été — moins la manipulation d'un produit dangereux, plus l'usage normal
du reste d'Internet.

Mais plus nous l'utilisions, plus nous butions sur des limites impossibles à
ignorer :

- une **clé de récupération générée dans un navigateur**, à laquelle il fallait
  simplement faire confiance ;
- **pas de réseaux personnalisés** ;
- **aucun moyen de l'héberger nous-mêmes** ;
- et le problème discret qui était pourtant le plus grave : **si le service
  disparaissait, le portefeuille disparaissait avec lui.**

Alors nous avons construit la version dont nous voulions dépendre.

## Ce qu'est Vela, concrètement

Vela est **un portefeuille à passkey que vous pouvez posséder entièrement.**

Votre passkey reste là où votre appareil la protège déjà — trousseau iCloud,
gestionnaire de mots de passe de Google, ou une clé de sécurité matérielle que vous
avez en main. Quand vous signez une transaction, Vela demande à votre appareil de
la signer ; votre appareil signe et ne renvoie que la signature. Vela ne voit
jamais la clé elle-même.

La plupart des portefeuilles gardent un moment dangereux, même bref : des mots à
l'écran, une phrase de récupération en mémoire, une clé de récupération posée dans
un onglet. Vela est conçu pour que ce moment n'existe pas.

<Callout type="info" title="Pas une promesse — une architecture">
Nous ne pouvons pas accéder à vos clés. Pas « nous promettons de ne pas le
faire » : il n'existe dans Vela aucun chemin de code qui le permette, et WebAuthn
ne le permet pas. Le portefeuille est un
<a href="/fr/docs/account-contract">compte intelligent Safe</a> actionné par une
signature que votre appareil produit et que nous ne faisons que recevoir. Ce que
décide, en revanche, l'app avec laquelle vous signez, c'est <em>ce que</em> l'on
demande à votre clé de signer — c'est pourquoi le
<a href="/fr/docs/whitepaper">modèle de menaces</a> s'y attarde autant.
</Callout>

Nous avons rendu Vela **open source** pour que vous puissiez le vérifier
vous-même, et **auto-hébergeable** pour qu'un portefeuille existant continue de
fonctionner sans les serveurs de notre entreprise — avec une limite, le domaine
auquel appartiennent vos passkeys, que le
[guide d'auto-hébergement](/fr/docs/self-hosting) explique, avec les moyens de la
contourner. Et nous l'avons bâti sur des
[contrats Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
non modifiés, parce que la voie ennuyeuse et éprouvée est la bonne quand il s'agit
de l'argent des gens — les mêmes contrats qui sécurisent déjà des milliards
on-chain.

## Le compromis que nous avons choisi

Il reste un compromis, et le passer sous silence serait malhonnête.

Avec Vela, votre compte Apple ou Google compte, parce que c'est là que vit une
passkey synchronisée. Perdez ce compte, ou supprimez la passkey, et il n'y a pas
de phrase de récupération, pas de réinitialisation par le support, pas de porte
dérobée.

Mais tout portefeuille en auto-conservation vous demande de choisir avec quel
risque vous préférez vivre. Une phrase de récupération peut être copiée, prise en
capture d'écran, hameçonnée, ou tapée sur le mauvais site à une heure du matin.
Une passkey, c'est différent : aucun mot à révéler, aucun secret à coller, et aucun
faux site capable de vous la soutirer. Votre navigateur ne la propose qu'aux pages
du vrai domaine.

Et le choix n'est pas binaire. Un portefeuille peut être créé avec **jusqu'à sept
signataires**, chacun capable de signer seul — des passkeys sur plusieurs
appareils, un téléphone à proximité que vous scannez, ou une clé de sécurité
USB/NFC. Si vous préférez que votre portefeuille ne dépende d'aucun compte de
plateforme, vous pouvez n'utiliser que des clés de sécurité matérielles — deux,
car un portefeuille ne peut pas reposer sur une seule clé qui n'est synchronisée
nulle part. La seule condition est le moment : votre adresse est dérivée de
l'ensemble des clés, elles se choisissent donc à la création du portefeuille.

<Callout type="warning" title="Ce que cela ne vous apporte pas">
Les signataires supplémentaires sont un chemin de retour, pas un second verrou.
Comme une seule clé suffit à signer, ajouter une clé matérielle vous protège contre
la <em>perte</em> de l'accès — cela n'arrête pas quelqu'un qui a déjà pris le
contrôle de l'une de vos clés. C'est la forme honnête du 1-of-n.
</Callout>

## Voilà pourquoi Vela existe

Un portefeuille sans phrase de récupération à cacher, sans clé de récupération à
laquelle se fier, et sans entreprise dont vous devez espérer qu'elle durera
toujours.

Si vous voulez vérifier ces affirmations plutôt que nous croire : le
[livre blanc](/fr/docs/whitepaper) décrit l'architecture,
[Audits et problèmes connus](/fr/docs/security-audits) liste chaque contrat dont
nous dépendons, avec ce qui a été audité et ce qui ne l'a pas été, et tout le code
est [sur GitHub](https://github.com/mondaylabsltd/vela-wallet).

Ensuite : [installer Vela](/fr/docs/install).
