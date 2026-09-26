---
title: Klartext-Signatur
description: "Vela übersetzt Transaktionen vor der Freigabe in verständliche Sprache – Absicht, Beträge, Adressen und Risiko – statt undurchsichtigem Hex. Kann es einen Aufruf nicht dekodieren, warnt es dich, statt so zu tun, als ob."
source: 7c184bfe125c
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Klartext-Signatur

Viele Wallets zeigen bei jedem Vertrag, den sie nicht kennen, noch immer Rohdaten an,
und „Blindsignieren“ – Aufrufe freigeben, die du eigentlich nicht lesen kannst – ist
einer der Wege, auf denen Wallets leergeräumt werden. Velas Antwort ist die
**Klartext-Signatur** (Clear Signing): Bevor du signierst, wird die Transaktion in
etwas übersetzt, das du verstehen kannst, soweit das möglich ist.

## Was du siehst

Statt roher Calldata zeigt Vela:

- **Absicht** – was die Transaktion tut: *Senden*, *Genehmigen*, *Tauschen* und so
  weiter.
- **Das Wesentliche** – die beteiligten Beträge und Adressen, Token-Beträge in echten
  Einheiten und Empfänger als Name, wo es einen gibt.
- **Die Details** – Nonce, Fristen und die rohe Calldata, auf Wunsch abrufbar statt
  ungefragt vor die Nase gesetzt.
- **Einen Risikohinweis**, farblich gekennzeichnet, damit gefährliche Aktionen
  auffallen.

## Wie es funktioniert (ERC-7730)

Vela dekodiert sowohl **Vertragsaufrufe** als auch **typisierte EIP-712-Daten** mit
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry)-Deskriptoren –
kleinen, teilbaren Definitionen dessen, was die Funktionen eines Vertrags bedeuten.

Vela sucht einen Deskriptor in dieser Reihenfolge:

1. **In der App eingebaut** – Deskriptoren für weit verbreitete Verträge: die Router
   von Uniswap, PancakeSwap und SushiSwap, WETH, den Aave-v3-Pool, 1inch, Lido und
   wstETH sowie Seaport.
2. **Vom Chain-Daten-Server von Vela abgerufen**, der das öffentliche
   ERC-7730-Verzeichnis neu veröffentlicht.
3. **Standardformen** – ERC-20-Token, ERC-721- und ERC-1155-NFTs, ERC-4626-Vaults und
   ERC-2612-Permits –, damit die meisten alltäglichen Aktionen trotzdem dekodiert
   werden.

**Verifiziert** ist der ersten Quelle vorbehalten. Als verifiziert gekennzeichnet
wird eine Transaktion nur, wenn die Beschreibung aus einem Deskriptor stammt, der in
die App eingebaut ist, die du gerade nutzt – oder vom Chain-Daten-Server kommt und mit
der eingebauten Kopie identisch ist, was belegt, dass unterwegs nichts verändert wurde.
Alles andere, was der Server schickt, wird trotzdem dekodiert und trotzdem angezeigt,
mit einer Zeile, die sagt, dass es vom Deskriptor-Dienst stammt und nichts es
authentifiziert hat. Dieser Dienst ist nicht signiert und damit nur so
vertrauenswürdig wie derjenige, der ihn betreibt – ein Grund, warum du
[einen eigenen betreiben](/de/docs/self-hosting#chain-data) kannst.

Token-Beträge werden mit den **echten On-Chain-Dezimalstellen** des Tokens
formatiert. Kann Vela die Dezimalstellen eines Tokens nicht bestätigen, zeigt es den
Betrag so an, als hätte der Token 18, und **kennzeichnet ihn als nicht verifiziert**,
damit eine falsche Zahl nie wie eine geprüfte aussieht.

## Risikostufen

Jede dekodierte Transaktion bekommt eine Risikostufe, damit gefährliche Muster
auffallen:

- **Vorsicht** bei Freigaben und Permits – du vergibst Ausgaberechte.
- **Gefahr** bei wirklich Riskantem, etwa einer **unbegrenzten Token-Freigabe**.
- Niedrigeres Risiko bei Routineaktionen wie Staking oder Einzahlungen.

<Callout type="warning" title="Eine „unbegrenzte“ Freigabe wird rot angezeigt, mit einer Obergrenze zur Auswahl">
Eine On-Chain-Freigabe über einen unbegrenzten Betrag ist einer der häufigsten Wege,
auf denen Guthaben später abgezogen wird. Fordert eine dApp eine solche Freigabe
(<code>approve</code>, <code>increaseAllowance</code> oder das <code>approve</code> von
Permit2) in „unbegrenzter“ Höhe an – 2^200 oder mehr (2^152 bei Permit2), also die
Werte, die dApps für „unbegrenzt“ verwenden –, zeigt Vela sie rot an und bietet eine
Obergrenze an: einen bestimmten Betrag, dein Guthaben (wenn es sich auslesen lässt)
oder – außer bei <code>increaseAllowance</code> – einen Widerruf. Wählst du nichts
davon, wird sie genau so gesendet, wie die dApp sie gebaut hat: Permit2 ist auf eine
dauerhafte Freigabe ausgelegt, und ein Batch eines Smart Accounts verbraucht die
Freigabe in derselben Transaktion, sodass eine Obergrenze unter diesem Betrag den
ganzen Batch scheitern lässt. Innerhalb eines Batches lässt sich jede Freigabe in den
iOS- und Android-Apps begrenzen; im Web und auf dem Desktop wird der Batch rot
markiert, ist aber noch nicht bearbeitbar. Eine letzte Prüfung vor dem Absenden liest
die rohe Calldata, sodass keine unbegrenzte Freigabe hinausgehen kann, die der
Freigabebildschirm nie gezeigt hat. Eine <strong>große, aber begrenzte Freigabe</strong>
(selbst weit über deinem Guthaben) wird mit einem Vorsichtshinweis angezeigt.
<strong>Signierte Permits</strong> (EIP-2612- und Permit2-Signaturen) lassen sich nicht
begrenzen – die dApp reicht ihre eigene Kopie ein –, deshalb werden sie wie angefragt
signiert oder abgelehnt: ein unbegrenztes in Rot, ein begrenztes mit einem
Vorsichtshinweis. Eine Anfrage, ein NFT-<code>setApprovalForAll</code> für eine ganze
Sammlung zu erteilen, lässt sich in den Apps noch nicht genehmigen.
</Callout>

## Wenn Vela einen Aufruf nicht dekodieren kann

Gibt es keinen ERC-7730-Deskriptor, steht die Funktion aber in einer öffentlichen
Selektor-Datenbank, dekodiert Vela den Aufruf generisch und kennzeichnet ihn unter
einem Vorsichtsbanner als **ohne Gewähr** – dekodiert, aber nicht verifiziert.
Scheitert auch das oder kann Vela nur einen Teil einer Transaktion dekodieren, tut es
**nicht** so, als würde es sie verstehen.

<Callout type="danger" title="Ausdrückliche Warnung vor Blindsignieren">
Lässt sich ein Aufruf nicht dekodieren, zeigt Vela eine deutliche Blindsignatur-Warnung
statt einer scheinbar freundlichen Zusammenfassung. Kann es nur einige Felder auflösen,
sagt es dir, dass die Ansicht unvollständig ist, und hält die Risikostufe erhöht. Du
weißt immer, wie viel von dem, was du signierst, Vela tatsächlich lesen konnte.
</Callout>

## Warum das wichtig ist

Selbstverwahrung heißt, dass niemand eine schlechte Transaktion für dich rückgängig
machen kann. Der Schutz ist kein Support-Schalter – sondern zu verstehen, was du
freigibst, **bevor** du es freigibst. Mit der Klartext-Signatur versucht Vela, dir das
zu zeigen, und sie hat Grenzen: Sie kann nur so ehrlich sein wie die App, die sie
anzeigt – deshalb ist eine [unabhängige Prüfung](/de/docs/clear-signing-self-host)
wichtig. Wo sie in Velas Sicherheitsmodell hingehört, steht im
[Whitepaper](/de/docs/whitepaper).
