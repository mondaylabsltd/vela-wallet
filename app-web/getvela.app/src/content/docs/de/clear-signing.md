---
title: Klartext-Signatur
description: Vela übersetzt Transaktionen vor der Freigabe in Klartext — Absicht, Beträge, Adressen und Risiko — statt undurchsichtiger Hex-Zeichen. Was es nicht dekodieren kann, sagt es dir, statt so zu tun als ob.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Klartext-Signatur

Die meisten Wallets zeigen dir eine Wand aus Hexadezimalzeichen und hoffen auf
das Beste. „Blindes Signieren“ — Aufrufe freigeben, die du nicht lesen kannst —
steckt hinter einem großen Teil der leergeräumten Wallets. Velas Antwort ist die
**Klartext-Signatur**: Vor dem Signieren wird die Transaktion in etwas übersetzt,
das du verstehen kannst.

## Was du siehst

Statt roher Calldata zeigt Vela:

- **Absicht** — was die Transaktion tut: *Senden*, *Genehmigen*, *Tauschen* und
  so weiter.
- **Die Substanz** — die beteiligten Beträge und Adressen, Token-Beträge in
  echten Einheiten und Empfänger zu einem Namen aufgelöst, wo es einen gibt.
- **Die Details** — Nonce, Fristen und die rohe Calldata, auf Abruf statt direkt
  ins Gesicht.
- **Einen Risikohinweis**, farbcodiert, damit das Gruselige gruselig aussieht.

## Wie es funktioniert (ERC-7730)

Vela dekodiert sowohl **Vertragsaufrufe** als auch **EIP-712-Typdaten** mithilfe
von [ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry)-Deskriptoren
— kleinen, teilbaren Definitionen dessen, was die Funktionen eines Vertrags
bedeuten.

- Existiert ein **vertragsspezifischer Deskriptor**, wird die Transaktion als
  **verifiziert** markiert und mit dem Namen des Vertrags versehen.
- Sonst fällt Vela auf **Standard-Deskriptoren** für gängige Formen zurück —
  ERC-20-Token, ERC-721-NFTs, ERC-4626-Vaults und ERC-2612-Permits — sodass die
  meisten Alltagsaktionen weiterhin dekodiert werden.

Token-Beträge werden mit den **echten On-Chain-Dezimalstellen** des Tokens
formatiert. Vela nimmt nie einfach 18 an; lassen sich die Dezimalstellen nicht
bestätigen, zeigt es den Wert, **markiert ihn aber als unverifiziert**, statt zu
raten.

## Risikostufen

Jede dekodierte Transaktion bekommt eine Risikostufe, damit die gefährlichen
Muster auffallen:

- **Achtung** bei Genehmigungen und Permits — du vergibst Ausgaberechte.
- **Gefahr** bei den wirklich riskanten Dingen, etwa einer **unbegrenzten
  Token-Genehmigung**.
- Geringeres Risiko bei Routine wie Staking oder Einzahlen.

<Callout type="warning" title="Unbegrenzte Genehmigungen werden blockiert">
Ein „approve“, das eine unbegrenzte Erlaubnis erteilt, ist einer der häufigsten
Wege, auf denen später Geld abfließt. Vela markiert das nicht nur: Es schreibt die
Anfrage auf einen endlichen Betrag deiner Wahl um, und eine letzte Prüfung vor dem
Absenden weigert sich, eine Genehmigung zu senden, die weiterhin unbegrenzt wäre.
Diese Sperre liest die rohe Calldata direkt, sie greift also auch dann, wenn es
für den Vertrag keinen Deskriptor gibt.
</Callout>

## Wenn Vela einen Aufruf nicht dekodieren kann

Ehrlichkeit zählt mehr als ein aufgeräumter Bildschirm. Gibt es keinen
ERC-7730-Deskriptor, taucht die Funktion aber in einer öffentlichen
Selector-Datenbank auf, dekodiert Vela generisch und kennzeichnet das Ergebnis
unter einem Achtung-Banner als **Best Effort** — dekodiert, aber nicht
verifiziert. Scheitert auch das, oder lässt sich nur ein Teil einer Transaktion
dekodieren, tut Vela **nicht** so, als hätte es sie verstanden.

<Callout type="danger" title="Ausdrückliche Blindsignatur-Warnung">
Lässt sich ein Aufruf nicht dekodieren, zeigt Vela eine klare
Blindsignatur-Warnung statt einer freundlich wirkenden Scheinzusammenfassung.
Lassen sich nur einige Felder auflösen, sagt es dir, dass die Ansicht
unvollständig ist, und hält die Risikostufe hoch. Du weißt immer, wie viel von
dem, was du signierst, Vela tatsächlich lesen konnte.
</Callout>

## Warum das wichtig ist

Selbstverwahrung heißt, dass niemand eine schlechte Transaktion für dich
rückgängig machen kann. Die Verteidigung ist kein Support-Schalter — sie besteht
darin, **vor** der Freigabe zu verstehen, was du freigibst. Klartext-Signatur
macht aus „vertrau diesem undurchsichtigen Klumpen“ ein „hier steht genau, was er
tut“. Wo das im Sicherheitsmodell von Vela sitzt, steht im
[Whitepaper](/de/docs/whitepaper).
