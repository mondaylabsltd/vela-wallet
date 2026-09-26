---
title: Héberger la page de signature
description: "La page sans dépendances, qui est aussi une extension Chrome, qui décode elle-même une transaction et la signe avec votre passkey — comment faire tourner votre propre copie, et quelle copie peut signer pour votre portefeuille."
source: c81389ef7aa2
---

# Héberger la page de signature

Vela décode chaque transaction avant que vous l'approuviez, et ce décodage est fait
honnêtement — mais il est fait par la même app que celle qui a construit la
transaction. Si l'app, ou la façon dont elle vous parvient, est altérée, elle peut
vous montrer une chose et en signer une autre. C'est exactement ce qui est arrivé à
[Bybit](/fr/docs/bybit-attack).

La page de signature existe pour séparer les deux : la transaction vient d'un
endroit, et la vérification comme la signature ont lieu à un endroit que vous
contrôlez.


Face à une demande de signature, elle ne se fie pas au résumé qui l'accompagne.
Elle décode elle-même la calldata brute, calcule son propre condensat, vous montre
ce que la signature autorisera réellement, et seulement ensuite sollicite votre
passkey.

Comme il n'y a pas d'étape de build, les fichiers que vous lisez sont les fichiers
qui s'exécutent. Vous pouvez comparer le dossier avec le dépôt et savoir exactement
ce que vous servez.

## Quelle copie peut signer pour votre portefeuille

Une passkey est liée au domaine sur lequel elle a été créée. Vos clés Vela sont
enregistrées sous `getvela.app`, et un navigateur ne les propose qu'à une page dont
la partie de confiance (relying party) est `getvela.app`. Cette seule règle décide
de la façon de faire tourner votre propre copie qui vous sera utile.

**Comme page sur votre propre domaine, ou sur localhost.** Servie en HTTPS (ou
depuis localhost), la page a pour partie de confiance son propre nom d'hôte — elle
peut donc signer avec des clés enregistrées sous _ce_ nom d'hôte, et non avec des
clés enregistrées sous `getvela.app`. C'est la bonne façon d'essayer toute la
cérémonie de bout en bout, de faire tourner le parcours de bureau, et de signer
pour un portefeuille dont la clé a été créée sur votre propre domaine. Ce n'est pas
un moyen de signer pour un portefeuille `getvela.app` existant.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Tous les chemins de l'app sont relatifs : un sous-répertoire sur un hôte existant
fonctionne donc aussi, et ouvrir `index.html` directement depuis le disque
(`file://`) permet de jeter un œil — sans origine, il n'y a pas de partie de
confiance, et rien ne peut être signé.

## Ce qu'elle fait avant de signer

- **Elle décode elle-même la transaction.** Ce que fait l'appel, pour qui et pour
  quel montant, à partir de la calldata — y compris les appels imbriqués dans un
  lot.
- **Elle ne signe qu'un condensat qu'elle a calculé.** Les condensats EIP-191,
  EIP-712, SafeOp et SafeMessage sont calculés dans la page et recoupés avec
  `vela-core`, le même code que celui du portefeuille. Un condensat qu'elle ne sait
  pas calculer donne lieu à un refus, pas à une signature.
- **Elle vérifie que la transaction est bien celle qui a été demandée.** L'appel
  demandé par le site doit réellement se trouver dans l'opération signée.
- **Elle signale quand une approbation est illimitée.** Elle ne peut pas modifier un
  montant — elle signe les octets reçus ou rien —, donc une approbation ou un permit
  illimité (2^128 ou plus sur cette page) s'affiche en rouge avec cette raison et
  peut être signé tel quel ; un plafond on-chain se choisit sur l'écran d'approbation
  du portefeuille lui-même, avant que la demande n'arrive ici. Une approbation
  portant sur toute une collection de NFT est refusée.
- **Elle dit quand elle ne parvient pas à lire quelque chose,** au lieu d'afficher
  un résumé rassurant qu'elle ne peut pas garantir.
- **Elle affiche l'adresse et l'identicon du compte,** et n'affiche pas un nom de
  destinataire fourni par celui qui demande la signature. Tout ce que le demandeur
  contrôle est soit écarté, soit présenté comme venant de lui.

## Ce qu'elle n'a pas, volontairement

- **Aucun éditeur.** La demande est figée à son arrivée : vous la signez ou non. Un
  sélecteur de frais ou un éditeur d'allocation réécrirait la calldata — c'est
  précisément le mal que cette page existe pour empêcher.
- **Aucune création de clé.** La page de signature ne peut pas créer de passkey.
  En créer une reviendrait à créer un autre compte.
- **Aucune donnée venue du réseau.** Rien de ce qu'elle affiche ou signe n'est
  récupéré à distance. La seule chose qu'elle charge, ce sont des logos de jetons,
  sous forme d'images, depuis le serveur de données de chaîne de Vela ; s'ils ne
  se chargent pas, une lettre les remplace.

## Comment une demande lui parvient

| Demandeur                                    | Canal                                                                      |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Une page dans le même navigateur             | `postMessage`                                                              |
| Une page dans le même navigateur, vers l'extension | Port d'extension                                                     |
| Une app de bureau sur la même machine        | Fragment d'URL + rappel en boucle locale (démo dans `samples/` ; l'app de bureau Vela ne l'utilise pas encore) |
| Un téléphone ou un autre ordinateur          | Bluetooth LE (protocole implémenté ; radio pas encore testée sur du vrai matériel) |

Le format d'échange, les condensats, et un tableau indiquant d'où vient chaque
élément affiché à l'écran se trouvent dans `PROTOCOL.md`, à côté du code.

## Sa place

Une fois que les apps pourront lui transmettre leurs demandes, l'usage prévu est
simple : dès le jour où le compte contient de l'argent que vous seriez fâché de
perdre, chaque signature passe par une page dont vous avez chargé le code
vous-même. Pas seulement pour les gros montants — une petite approbation peut
suffire à livrer de quoi vider un compte. D'ici là, la page est un moyen de lire et
de tester exactement comment fonctionnera ce second avis.
