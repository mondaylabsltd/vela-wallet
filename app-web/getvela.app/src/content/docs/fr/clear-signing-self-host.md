---
title: Héberger la page de signature
description: La page sans dépendances et l'extension Chrome qui décodent une transaction elles-mêmes et la signent avec votre passkey — comment faire tourner votre propre copie, et quelle copie peut signer pour votre portefeuille.
---

# Héberger la page de signature

Vela décode chaque transaction avant que vous ne l'approuviez, et ce travail est
un vrai travail — mais il est fait par l'application qui a construit la
transaction. Si l'application, ou le chemin par lequel elle vous parvient, est
altérée, elle peut vous montrer une chose et en signer une autre. C'est exactement
ce qui est arrivé à [Bybit](/fr/docs/bybit-attack).

La page de signature existe pour couper cela en deux : la transaction vient d'un
endroit, la vérification et la signature se passent à un autre, que vous
contrôlez.

## Ce que c'est

Un dossier — `app-web/trusted-signer` dans le dépôt — qui est à la fois une page web
et une extension Chrome. Du HTML, du CSS et du JavaScript purs : pas de framework,
pas de bundler, pas d'étape de build, pas de dépendances, et aucune requête réseau
de son propre chef.

Face à une demande de signature, elle ne fait pas confiance au résumé livré avec.
Elle décode elle-même la calldata brute, calcule son propre condensé, vous montre
ce que la signature autorisera réellement, et seulement ensuite demande votre
passkey.

Comme il n'y a pas d'étape de build, les fichiers que vous lisez sont ceux qui
s'exécutent. Vous pouvez comparer le dossier au dépôt et savoir ce que vous
servez.

## Quelle copie peut signer pour votre portefeuille

Une passkey est liée au domaine sur lequel elle a été créée. Vos clés Vela sont
enregistrées sous `getvela.app`, et un navigateur ne les proposera qu'à une page
dont la relying party est `getvela.app`. Cette seule règle décide de la manière
utile de faire tourner votre copie.

**En extension Chrome — celle à utiliser avec votre portefeuille existant.** La
relying party de l'extension est `getvela.app` quelle que soit la provenance du
dossier : vos clés existantes peuvent donc y signer, tandis que le code exécuté
est le dossier que vous avez chargé et inspecté.

1. Ouvrez `chrome://extensions` et activez le **mode développeur**.
2. **Charger l'extension non empaquetée**, puis choisissez le dossier
   `app-web/trusted-signer`.
3. L'icône de la barre d'outils ouvre la page dans un onglet.

**En page sur votre propre domaine, ou sur localhost.** Servie en HTTP(S), la
relying party de la page est son propre nom d'hôte — elle peut donc signer avec
des clés enregistrées sous *ce* nom d'hôte, pas avec des clés enregistrées sous
`getvela.app`. C'est la bonne façon d'essayer toute la cérémonie de bout en bout,
de faire tourner le flux bureau, et de signer pour un portefeuille dont la clé a
été créée sur votre propre domaine. Ce n'est pas une façon de signer pour un
portefeuille `getvela.app` existant.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Tous les chemins de l'application sont relatifs : un sous-répertoire sur un hôte
existant fonctionne aussi. Ouvrir `index.html` directement depuis le disque
(`file://`) permet de regarder — sans origine, il n'y a pas de relying party et
rien ne peut être signé.

## Ce qu'elle fait avant de signer

- **Elle décode la transaction elle-même.** Ce que fait l'appel, pour qui et pour
  combien, à partir de la calldata — y compris les appels imbriqués dans un lot.
- **Elle ne signe qu'un condensé qu'elle a calculé.** Les condensés EIP-191,
  EIP-712, SafeOp et SafeMessage sont calculés dans la page et recoupés avec
  `vela-core`, le même code que celui du portefeuille. Un condensé qu'elle ne peut
  pas calculer est un refus, pas une signature.
- **Elle vérifie que la transaction est bien celle demandée.** L'appel réclamé par
  le site doit réellement se trouver dans l'opération signée.
- **Elle refuse une approbation illimitée.** Pas un avertissement — un refus, avec
  une indication de ce qu'il faut faire à la place.
- **Elle dit quand elle ne peut pas lire quelque chose,** au lieu d'afficher un
  résumé sympathique qu'elle ne peut pas assumer.
- **Elle affiche l'adresse et l'identicon du compte,** et n'affiche pas un nom de
  destinataire fourni par celui qui demande la signature. Tout ce que le demandeur
  contrôle est soit retiré, soit étiqueté comme venant de lui.

## Ce qu'elle n'a délibérément pas

- **Aucun éditeur.** La demande est figée à son arrivée : vous la signez ou non.
  Un sélecteur de frais ou un éditeur de plafond réécrirait la calldata, ce qui
  est exactement la maladie que cette page existe pour empêcher.
- **Aucune création de clé.** La page de signature ne peut pas créer de passkey.
  En créer une reviendrait à créer un autre compte.
- **Aucune requête réseau.** Rien à aller chercher, donc rien à intercepter.

## Comment une demande lui parvient

| Demandeur | Canal |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Une page dans le même navigateur | `postMessage` |
| Une page du même navigateur, vers l'extension | Port d'extension |
| Une application de bureau sur la même machine | Fragment d'URL + rappel en loopback |
| Un téléphone ou un autre ordinateur | Bluetooth LE (protocole implémenté ; radio pas encore testée sur du vrai matériel) |

Le format d'échange, les condensés et un tableau indiquant d'où vient chaque
élément affiché sont dans `PROTOCOL.md`, à côté du code.

## Quand l'utiliser

Le jour où le compte détient de l'argent dont la perte vous ennuierait — et à
partir de là, pour chaque signature. Pas seulement pour les gros montants : une
petite approbation peut céder de quoi vider un compte. Une habitude de signature
gardée pour les grandes occasions n'est pas en place le jour où elle sert.
