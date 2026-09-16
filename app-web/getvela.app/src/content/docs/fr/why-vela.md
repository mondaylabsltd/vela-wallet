---
title: Pourquoi nous avons créé Vela
description: La version longue — où êtes-vous censé garder douze mots, ce que les passkeys ont changé, ce que nous ne pouvions pas accepter dans les portefeuilles que nous utilisions déjà, et le compromis que nous avons choisi à la place.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Pourquoi nous avons créé Vela

Nous ne voulions pas faire un portefeuille de plus. Tout est parti d'une question
à laquelle nous n'avons jamais su répondre proprement :

> Où êtes-vous censé garder douze mots ?

## La réponse honnête, c'est une capture d'écran

Mettez-les dans une note et vous êtes à un téléphone volé de l'accident.
Écrivez-les sur papier, et vous voilà à penser au feu, à l'eau, aux déménagements,
aux colocataires, aux sacs-poubelle, et à savoir si votre futur vous se souviendra
de « l'endroit sûr ».

Pour beaucoup, la réponse honnête est une capture d'écran dans la pellicule. Tout
le monde sait que c'est une mauvaise idée. Tout le monde le fait quand même —
parce que la « bonne » réponse est trop lourde à vivre.

Une phrase de récupération, c'est un secret qui doit survivre à des décennies de
vie ordinaire sans jamais être copié, photographié, tapé dans le mauvais champ ou
lu à voix haute à quelqu'un de serviable au téléphone. Ce n'est pas un problème
difficile pour une personne très prudente. C'est un problème difficile pour une
personne.

## Puis les passkeys ont changé ce qu'un portefeuille peut être

Nous utilisions [Base Account](https://account.base.app) tous les jours, et signer
avec Face ID paraissait évident d'une manière que les phrases de récupération
n'ont jamais eue — moins la manipulation d'un produit dangereux, plus le reste
d'Internet.

Mais plus nous l'utilisions, plus nous heurtions des bords impossibles à ignorer :

- une **clé de récupération générée dans un navigateur** qu'il fallait simplement
  croire sur parole,
- **pas de réseaux personnalisés**,
- **aucun moyen de l'héberger nous-mêmes**,
- et le problème silencieux qui était le plus gros : **si le service disparaît, le
  portefeuille disparaît avec lui.**

Alors nous avons construit la version dont nous voulions dépendre.

## Ce qu'est Vela, concrètement

Vela est **un portefeuille à passkey que vous pouvez posséder entièrement.**

Votre passkey reste là où votre appareil la protège déjà — trousseau iCloud,
gestionnaire de mots de passe Google, ou une clé de sécurité matérielle que vous
tenez. Quand vous signez une transaction, Vela envoie un défi à votre appareil ;
votre appareil le signe et ne renvoie que la signature. Vela ne voit jamais la clé
elle-même.

La plupart des portefeuilles gardent un moment dangereux, même bref : des mots à
l'écran, une phrase de récupération en mémoire, une clé de récupération posée dans
un onglet. Vela est conçu pour que ce moment n'existe pas.

<Callout type="info" title="Pas une promesse — une architecture">
Nous ne pouvons pas accéder à vos clés. Pas « nous promettons de ne pas le
faire » : il n'existe dans Vela aucun chemin de code qui le permette. Le
portefeuille est un
<a href="/fr/docs/security-audits">compte intelligent Safe</a> opéré par une
signature que votre appareil produit et que nous ne faisons que recevoir.
</Callout>

Nous avons rendu Vela **open source** pour que vous puissiez le vérifier
vous-même, et **auto-hébergeable** pour que votre portefeuille ne dépende jamais
du fait que notre entreprise reste en ligne. Et nous l'avons bâti sur des
[contrats Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
non modifiés, parce que la voie ennuyeuse et éprouvée est la bonne quand il s'agit
de l'argent des gens — les mêmes contrats qui sécurisent déjà des milliards
on-chain.

## Le compromis que nous avons choisi

Il reste un compromis, et l'enterrer serait malhonnête.

Avec Vela, votre compte Apple ou Google compte, parce que c'est là que vit une
passkey synchronisée. Perdez ce compte, ou supprimez la passkey, et il n'y a pas
de phrase de récupération, pas de réinitialisation par le support, pas de porte
dérobée.

Mais tout portefeuille auto-conservé vous demande de choisir avec quel risque vous
préférez vivre. Une phrase de récupération peut être copiée, prise en capture,
hameçonnée, ou tapée sur le mauvais site à une heure du matin. Une passkey, c'est
différent : aucun mot à révéler, aucun secret à coller, et aucun faux site capable
de vous la faire livrer. Votre appareil signe pour le vrai domaine, ou il ne signe
pas.

Et le choix n'est pas binaire. Un portefeuille peut être créé avec **jusqu'à sept
signataires**, chacun capable de signer seul — des passkeys sur plusieurs
appareils, un téléphone à proximité que vous scannez, ou une clé de sécurité
USB/NFC. Si vous préférez que votre portefeuille ne dépende d'aucun compte de
plateforme, vous pouvez faire de la toute première clé une clé de sécurité
matérielle. La seule condition est le moment : votre adresse est dérivée de
l'ensemble des clés, elles se choisissent donc à la création.

<Callout type="warning" title="Ce que cela ne vous achète pas">
Les signataires supplémentaires sont un chemin de retour, pas un second verrou.
Comme une seule clé suffit à signer, ajouter une clé matérielle vous protège de
<em>perdre</em> l'accès — cela n'arrête pas quelqu'un qui a déjà pris le contrôle
de l'une de vos clés. C'est la forme honnête du 1-of-n.
</Callout>

## Voilà pourquoi Vela existe

Un portefeuille sans phrase de récupération à cacher, sans clé de récupération à
croire, et sans entreprise dont vous devez espérer qu'elle durera toujours.

Si vous voulez vérifier plutôt que croire : le
[livre blanc](/fr/docs/whitepaper) contient l'architecture,
[audits et problèmes connus](/fr/docs/security-audits) liste chaque contrat dont
nous dépendons et ce qui a été audité ou non, et tout le code est
[sur GitHub](https://github.com/mondaylabsltd/vela-wallet).

Ensuite : [installer Vela](/fr/docs/install).
