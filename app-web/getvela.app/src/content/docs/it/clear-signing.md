---
title: Firma leggibile
description: Vela decodifica le transazioni in parole chiare prima che tu le approvi — intenzione, importi, indirizzi e rischio — invece di esadecimale opaco. Quando non riesce a decodificare una chiamata, ti avvisa invece di fingere.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Firma leggibile

La maggior parte dei wallet ti chiede di approvare un muro di esadecimale e di
sperare bene. La «firma al buio» — approvare chiamate che non sai leggere — sta
dietro a una bella fetta dei wallet svuotati. La risposta di Vela è la **firma
leggibile**: prima che tu firmi, la transazione viene tradotta in qualcosa che
puoi capire.

## Cosa vedi

Al posto della calldata grezza, Vela mostra:

- **L'intenzione** — cosa fa la transazione: *Invia*, *Approva*, *Scambia* e così
  via.
- **La sostanza** — importi e indirizzi coinvolti, con gli importi dei token in
  unità reali e i destinatari risolti in un nome dove ne esiste uno.
- **I dettagli** — nonce, scadenze e calldata grezza, disponibili su richiesta
  invece che sbattuti in faccia.
- **Un'indicazione di rischio**, a colori, così le cose spaventose sembrano
  spaventose.

## Come funziona (ERC-7730)

Vela decodifica sia le **chiamate a contratto** sia i **dati tipizzati EIP-712**
usando i descrittori
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry): piccole
definizioni condivisibili di cosa significano le funzioni di un contratto.

- Quando esiste un **descrittore specifico del contratto**, la transazione viene
  marcata **verificata** ed etichettata con il nome del contratto.
- Quando non c'è, Vela ripiega su **descrittori standard** per le forme comuni —
  token ERC-20, NFT ERC-721, vault ERC-4626 e permit ERC-2612 — così la maggior
  parte delle azioni quotidiane si decodifica comunque.

Gli importi dei token sono formattati con i **decimali reali on-chain** del token.
Vela non dà mai per scontato 18; se non riesce a confermare i decimali mostra il
valore ma **lo segnala come non verificato**, invece di tirare a indovinare.

## Livelli di rischio

Ogni transazione decodificata riceve un livello di rischio, così gli schemi
pericolosi saltano all'occhio:

- **Attenzione** per approvazioni e permit: stai concedendo potere di spesa.
- **Pericolo** per ciò che è davvero rischioso, come un'**approvazione di token
  illimitata**.
- Rischio più basso per azioni di routine come staking o deposito.

<Callout type="warning" title="Le approvazioni illimitate vengono bloccate">
Un «approve» che concede un'allowance illimitata è uno dei modi più comuni in cui i
fondi finiscono per essere prosciugati. Vela non si limita a segnalarlo: riscrive la
richiesta su un importo finito che scegli tu, e un ultimo controllo prima dell'invio
rifiuta qualsiasi approvazione che resterebbe illimitata. Quel controllo legge
direttamente la calldata grezza, quindi funziona anche quando per quel contratto non
esiste alcun descrittore.
</Callout>

## Quando Vela non riesce a decodificare una chiamata

L'onestà conta più di una schermata pulita. Quando non esiste un descrittore
ERC-7730 ma la funzione compare in un database pubblico di selettori, Vela decodifica
la chiamata in modo generico e la etichetta come **best effort** — decodificata, ma
non verificata — sotto un banner di attenzione. Se anche questo fallisce, o se Vela
riesce a decodificare solo una parte della transazione, **non** fa finta di averla
capita.

<Callout type="danger" title="Avviso esplicito di firma al buio">
Se una chiamata non si può decodificare, Vela mostra un chiaro avviso di firma al
buio invece di un riassunto finto-rassicurante. Se riesce a risolvere solo alcuni
campi, ti dice che la vista è parziale e tiene alto il livello di rischio. Sai
sempre quanto di ciò che stai firmando Vela è riuscito davvero a leggere.
</Callout>

## Perché conta

Auto-custodia significa che nessuno può annullare una transazione sbagliata al
posto tuo. La difesa non è un servizio clienti: è capire cosa approvi **prima** di
approvarlo. La firma leggibile trasforma «fidati di questo blob opaco» in «ecco
esattamente cosa fa». Dove si colloca nel modello di sicurezza complessivo lo
trovi nel [whitepaper](/it/docs/whitepaper).
