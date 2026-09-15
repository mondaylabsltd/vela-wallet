---
title: L'attaque de Bybit
description: En février 2025, Bybit a perdu environ 1,5 milliard de dollars. Les contrats Safe n'étaient pas cassés — l'interface l'était. Cette page explique le chemin emprunté, et ce que le design de Vela y ferme.
---

# L'attaque de Bybit

Le 21 février 2025, Bybit a perdu environ **1,5 milliard de dollars** depuis un
portefeuille froid multisig Safe. C'est le plus grand vol de l'histoire du secteur,
et il mérite une lecture attentive, parce que presque tout y était *correct*, sauf
une chose.

## Ce qui s'est passé

La version courte, d'après les post-mortems publics :

1. Un attaquant a compromis la **machine d'un développeur de `Safe{Wallet}`** et
   injecté du JavaScript malveillant dans le bucket AWS S3 qui servait le
   frontend `Safe{Wallet}`. Le code est entré le 19 février et s'est déclenché le
   21, ciblant précisément le Safe de Bybit.
2. Les signataires de Bybit ont ouvert l'interface et examiné une transaction qui
   paraissait ordinaire.
3. La charge réellement envoyée à leurs **portefeuilles matériels** n'était pas
   cette transaction. C'était un `delegatecall` qui écrasait la `masterCopy` du
   proxy Safe — l'emplacement 0 — remplaçant toute l'implémentation du compte par
   celle de l'attaquant.
4. Les signataires ont approuvé. Les signatures étaient valides. Le contrat a fait
   exactement ce qu'on lui demandait.

L'attribution publique a pointé une activité liée à la Corée du Nord (le FBI a
nommé le groupe TraderTraitor).

## Ce qui n'était *pas* cassé

- **Pas les contrats Safe.** Ils ont exécuté une instruction valablement signée.
  Aucun bug de Safe n'a été exploité.
- **Pas la cryptographie.** Chaque signature était authentique.
- **Pas les portefeuilles matériels.** Des appareils Ledger étaient dans la boucle
  et ont signé quand même — parce qu'un portefeuille matériel montre ce qu'on lui
  donne, et ce qu'on lui a donné, c'était la charge malveillante. Un appareil
  incapable de traduire un `delegatecall` en quelque chose qu'un humain peut juger
  protège la *clé*, pas la *décision*.

Ce qui était cassé, c'est l'hypothèse sous toute interface de portefeuille :
**que l'écran décrivant une transaction et les octets signés sont la même chose.**

## Pourquoi c'est le cas général, pas un accident isolé

Chaque signature que vous avez produite dans un portefeuille web reposait sur
cette hypothèse. L'interface construit la charge, l'interface dessine le résumé,
et rien d'indépendant ne vérifie que les deux correspondent. Si le code qui sert
cette interface est remplacé — chaîne de build compromise, CDN détourné,
dépendance malveillante, identifiant de déploiement volé — le résumé devient ce
que l'attaquant veut, et votre signature est bien réelle.

C'est ce risque que vise le design de signature de Vela. Pas l'hameçonnage. Pas
une clé fuitée. **Un écran de signature qui vous ment.**

## Ce que fait Vela

**La signature lisible, jusqu'à la calldata.** Chaque transaction est traduite en
intention lisible avant approbation — montant, destinataire, ce que l'appel fait
réellement ([ERC-7730](/fr/docs/clear-signing)). Un appel que nous ne savons pas
décoder est **signalé comme indécodable**, pas rendu discrètement comme s'il allait
de soi. La charge de Bybit était un `delegatecall` qui échangeait une adresse
d'implémentation ; c'est précisément le genre de chose qui doit arrêter net un
signataire, et la cacher derrière un résumé aimable est la raison pour laquelle
elle ne l'a pas fait.

**Un chemin indépendant capable de contrôler l'interface.** Vela construit une
page de signature sans build ni dépendances, qui affiche l'intention et effectue
la signature WebAuthn par elle-même — un dossier unique de fichiers statiques que
vous pouvez lire de bout en bout, servir vous-même, ou lancer comme extension de
navigateur. Son objet est d'être un second avis qui ne partage pas la chaîne
d'approvisionnement de l'application principale. *État : construite et testée,
pas encore déployée.* À sa sortie, elle sera optionnelle, et cette page le dira
clairement quand cela changera.

**Aucun contrat que nous puissions mettre à niveau.** La charge de Bybit a
fonctionné en remplaçant l'implémentation du compte. Les comptes de Vela sont des
[Safe v1.4.1 non modifiés](/fr/docs/account-contract), et Vela n'y détient aucun
rôle privilégié — pas de clé d'administration, pas de chemin de mise à niveau
qu'on pourrait nous forcer ou nous compromettre à utiliser.

**Une vérification biométrique fraîche à chaque signature.** Il n'y a pas de clé
de session longue durée, donc pas de fenêtre pendant laquelle quelque chose
pourrait signer à votre place sans vous.

**L'auto-hébergement comme filet.** L'application et chaque service backend sont
open source. Si vous ne voulez pas faire confiance du tout à notre chaîne de
build, faites tourner la vôtre — c'est la seule réponse à cette classe d'attaque
qui n'exige de faire confiance à personne.

## Ce que Vela ne prétend pas

Le frontend de Vela pourrait être compromis de la même façon que celui de
`Safe{Wallet}`. Notre code n'est pas audité. Dire autre chose serait exactement le
genre d'assurance auquel cet incident aurait dû mettre fin.

Ce que le design tente, c'est de rétrécir le chemin : rendre la charge lisible au
lieu d'opaque, retirer la primitive de mise à niveau sur laquelle l'attaque s'est
appuyée, et vous donner un moyen de vérifier avec autre chose que nous. Le résumé
honnête : **cette classe d'attaque est atténuée par conception, pas éliminée** —
et les pièces qui la durciraient davantage sont listées, inachevées, dans
[audits et problèmes connus](/fr/docs/security-audits).

## Sources

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
