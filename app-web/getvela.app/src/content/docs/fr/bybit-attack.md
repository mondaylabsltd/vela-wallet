---
title: L'attaque de Bybit, et le chemin qu'elle a emprunté
description: "En février 2025, Bybit a perdu environ 1,5 milliard de dollars. Les contrats Safe n'ont pas été cassés — c'est l'interface qui l'a été. Cette page explique le chemin emprunté, et ce qui, dans la conception de Vela, le ferme."
source: ac56b16b531f
---

# L'attaque de Bybit, et le chemin qu'elle a emprunté

Le 21 février 2025, Bybit a perdu environ **1,5 milliard de dollars** depuis un
portefeuille froid multisig Safe. C'est le plus grand vol de l'histoire du secteur,
et il mérite une lecture attentive, parce que presque tout y était *correct*, sauf
une chose.

## Ce qui s'est passé

La version courte, d'après les analyses post-mortem publiques :

1. Un attaquant a compromis la **machine d'un développeur de `Safe{Wallet}`** et
   injecté du JavaScript malveillant dans le bucket AWS S3 qui servait le frontend
   de `Safe{Wallet}`. Le code a été placé le 19 février et déclenché le 21, en
   ciblant précisément le Safe de Bybit.
2. Les signataires de Bybit ont ouvert l'interface et examiné une transaction qui
   paraissait ordinaire.
3. La charge utile réellement envoyée à leurs **portefeuilles matériels** n'était
   pas cette transaction. C'était un `delegatecall` qui écrasait la `masterCopy` du
   proxy Safe — l'emplacement 0 —, remplaçant toute l'implémentation du compte par
   celle de l'attaquant.
4. Les signataires ont approuvé. Les signatures étaient valides. Le contrat a fait
   exactement ce qu'on lui demandait.

L'attaque a été publiquement attribuée à une activité liée à la Corée du Nord (le
FBI a nommé le groupe TraderTraitor).

## Ce qui n'a *pas* été cassé

- **Pas les contrats Safe.** Ils ont exécuté une instruction valablement signée.
  Aucun bug de Safe n'a été exploité.
- **Pas la cryptographie.** Chaque signature était authentique.
- **Pas les portefeuilles matériels.** Des appareils Ledger étaient dans la boucle
  et ont signé quand même — parce qu'un portefeuille matériel montre ce qu'on lui
  donne, et ce qu'on lui a donné, c'était la charge utile malveillante. Un appareil
  incapable de traduire un `delegatecall` en quelque chose qu'un humain peut juger
  protège la *clé*, pas la *décision*.

Ce qui a été cassé, c'est l'hypothèse sur laquelle repose toute interface de
portefeuille : **que l'écran qui décrit une transaction et les octets signés sont
une seule et même chose.**

## Pourquoi c'est le cas général, pas un accident isolé

La plupart des signatures produites dans un portefeuille web reposent sur cette
hypothèse. L'interface construit la charge utile, l'interface affiche le résumé, et
rien d'indépendant ne vérifie que l'un correspond à l'autre. Si le code qui sert
cette interface est remplacé — par une chaîne de build compromise, un CDN détourné,
une dépendance malveillante, un identifiant de déploiement volé —, le résumé devient
ce que l'attaquant veut, et votre signature, elle, est bien réelle.

C'est ce risque que vise la conception de la signature dans Vela. Pas
l'hameçonnage. Pas une clé divulguée. **Un écran de signature qui vous ment.**

## Ce que fait Vela

**La signature lisible, jusqu'à la calldata.** Chaque transaction est décodée en
intention lisible avant que vous l'approuviez — montant, destinataire, ce que
l'appel fait réellement ([ERC-7730](/fr/docs/clear-signing)). Un appel que nous ne
savons pas décoder est **signalé comme indécodable**, pas affiché discrètement
comme si tout allait bien. La charge utile de Bybit était un `delegatecall` qui
remplaçait une adresse d'implémentation ; c'est précisément le genre de chose qui
devrait arrêter net un signataire, et c'est en la cachant derrière un résumé
rassurant qu'on l'en a empêché.

Deux limites à préciser. Une dApp ne peut pas demander directement un
`delegatecall` à Vela — les demandes qu'une page peut faire produisent des appels
ordinaires —, donc la charge utile de Bybit elle-même ne pourrait pas arriver par
ce chemin. Mais une page *peut* demander un appel de votre Safe vers lui-même :
`enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`.
N'importe lequel de ces appels, signé une seule fois, livre le compte aussi
complètement que la charge utile de Bybit — un module activé peut ensuite exécuter son
propre `delegatecall`. Vela décode ces appels mais ne les bloque pas encore ;
**refusez toute demande dont la cible est l'adresse de votre propre portefeuille.**
Et si le code de Vela lui-même était remplacé, comme l'a été celui de
`Safe{Wallet}`, le décodage serait lui aussi celui de l'attaquant — c'est à cela que
sert le point suivant.

**Un chemin indépendant capable de contrôler l'interface.** Vela a construit une
[page de signature](/fr/docs/clear-signing-self-host) sans build ni dépendances,
qui décode la demande et effectue la signature WebAuthn par elle-même — un unique
dossier de fichiers statiques que vous pouvez lire de bout en bout, servir
vous-même, ou charger comme extension de navigateur. Son rôle est d'être un second
avis qui ne partage pas la chaîne d'approvisionnement de l'app principale.
*État : construite et testée ; pas publiée, et aucune app Vela ne lui envoie encore
de demandes.* Cette page le dira clairement quand cela changera.

**Aucun rôle d'administrateur que l'on pourrait nous voler.** Les comptes Vela sont
des [Safe v1.4.1 non modifiés](/fr/docs/account-contract), et Vela n'y détient
aucun rôle privilégié — pas de clé d'administration, ni de chemin de mise à niveau
à nous, qu'on pourrait nous contraindre ou nous piéger à utiliser. Soyons clairs sur
ce que cela *n'élimine pas* : la primitive utilisée par les attaquants de Bybit — un
`delegatecall` signé par un propriétaire, qui réécrit l'implémentation du compte —
existe toujours dans chaque Safe, y compris ceux de Vela (les transactions groupées
de Vela passent elles-mêmes par un `delegatecall` vers le MultiSend de Safe). Elle
exige une signature valide de l'une de vos clés. Les défenses contre le risque
qu'on vous en soutire une sont le décodage ci-dessus et la vérification
indépendante.

**Une nouvelle vérification à chaque signature.** Chaque signature exige la
confirmation propre à votre clé — Face ID, une empreinte, un code PIN, ou un appui
et un code PIN sur une clé de sécurité. Il n'y a pas de clé de session longue
durée, donc pas de fenêtre pendant laquelle quelque chose pourrait signer à votre
place sans vous.

**L'auto-hébergement en dernier recours.** Les apps et les services backend sont
open source. Si vous ne voulez pas du tout faire confiance à notre chaîne de build,
compilez vous-même l'extension ou une app et faites tourner les services dont vous
avez besoin — le [guide d'auto-hébergement](/fr/docs/self-hosting) vous guide pas à
pas. Notre chaîne de build sort alors de la chaîne de confiance ; vous faites
toujours confiance au code que vous compilez, alors lisez-le.

## Ce que Vela ne prétend pas

Le frontend de Vela pourrait être compromis de la même façon que celui de
`Safe{Wallet}`. Notre code n'est pas audité. Prétendre le contraire serait
exactement le genre d'assurance auquel cet incident aurait dû mettre fin.

Ce que la conception essaie de faire, c'est de rétrécir le chemin : rendre la
charge utile lisible au lieu d'opaque, ne détenir aucun rôle d'administrateur dont on
pourrait abuser à vos dépens, et vous donner un moyen de vérifier avec autre chose
que nous. Le résumé honnête : **cette classe d'attaque est atténuée par conception,
pas éliminée**, et ce qui la durcirait davantage est listé, inachevé, dans
[audits et problèmes connus](/fr/docs/security-audits).

## Sources

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
