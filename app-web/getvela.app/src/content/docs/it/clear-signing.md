---
title: Firma leggibile
description: "Vela decodifica le transazioni in linguaggio chiaro prima che tu le approvi — intento, importi, indirizzi e rischio — invece di esadecimale incomprensibile. Quando non riesce a decodificare una chiamata, ti avvisa invece di fingere."
source: 7c184bfe125c
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Firma leggibile

Molti wallet mostrano ancora dati grezzi per qualsiasi contratto che non
riconoscono, e la «firma alla cieca» — approvare chiamate che non sai davvero
leggere — è uno dei modi in cui i wallet vengono svuotati. La risposta di Vela è
la **firma leggibile**: prima che tu firmi, la transazione viene decodificata in
qualcosa che puoi capire, per quanto è possibile.

## Cosa vedi

Al posto della calldata grezza, Vela mostra:

- **L'intento** — cosa fa la transazione: *Invia*, *Approva*, *Scambia* e così via.
- **La sostanza** — gli importi e gli indirizzi coinvolti, con gli importi dei
  token in unità reali e i destinatari risolti in un nome dove ne esiste uno.
- **I dettagli** — nonce, scadenze e calldata grezza, disponibili su richiesta
  invece che sbattuti in faccia.
- **Un'indicazione di rischio**, a colori, così le azioni pericolose saltano
  all'occhio.

## Come funziona (ERC-7730)

Vela decodifica sia le **chiamate ai contratti** sia i **dati tipizzati EIP-712**
usando i descrittori
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry): piccole
definizioni condivisibili di cosa significano le funzioni di un contratto.

Vela cerca un descrittore in quest'ordine:

1. **Integrato nell'app** — descrittori per i contratti più usati: i router di
   Uniswap, PancakeSwap e SushiSwap, WETH, il pool di Aave v3, 1inch, Lido e
   wstETH, e Seaport.
2. **Scaricato dal server dei dati delle chain di Vela**, che ripubblica il
   registro pubblico ERC-7730.
3. **Forme standard** — token ERC-20, NFT ERC-721 ed ERC-1155, vault ERC-4626 e
   permit ERC-2612 — così la maggior parte delle azioni quotidiane si decodifica
   comunque.

**Verificata** è riservata alla prima fonte. Una transazione viene etichettata
come verificata solo quando la descrizione arriva da un descrittore integrato
nell'app che stai usando — oppure dal server dei dati delle chain ed è identica
alla copia integrata, il che dimostra che per strada non è stato cambiato nulla.
Tutto il resto di ciò che il server manda viene comunque decodificato e comunque
mostrato, con una riga che dice che arriva dal servizio dei descrittori e che
nulla lo ha autenticato. Quel servizio non è firmato, quindi è affidabile solo
quanto chi lo gestisce — ed è uno dei motivi per cui puoi
[gestirne uno tuo](/it/docs/self-hosting#chain-data).

Gli importi dei token sono formattati con i **decimali reali on-chain** del token.
Se Vela non riesce a confermare i decimali di un token, mostra l'importo come se
il token ne avesse 18 e **lo segna come non verificato**, così un numero sbagliato
non sembra mai un numero controllato.

## Livelli di rischio

Ogni transazione decodificata riceve un livello di rischio, così gli schemi
pericolosi saltano all'occhio:

- **Attenzione** per approvazioni e permit: stai concedendo un potere di spesa.
- **Pericolo** per ciò che è davvero rischioso, come un'**approvazione di token
  illimitata**.
- Rischio più basso per azioni di routine come staking o depositi.

<Callout type="warning" title="Un'approvazione «illimitata» appare in rosso, con un limite proposto">
Un'approvazione on-chain per un importo illimitato è uno dei modi più comuni in cui
i fondi vengono prosciugati in seguito. Quando una dApp ne chiede una
(<code>approve</code>, <code>increaseAllowance</code> o l'<code>approve</code> di
Permit2) al livello «illimitato» — 2^200 o più (2^152 per Permit2), che è ciò che
le dApp usano per dire «illimitato» — Vela la mostra in rosso e propone un limite:
un importo preciso, il tuo saldo (quando Vela riesce a leggerlo) o — tranne che con
<code>increaseAllowance</code> — una revoca. Se non ne scegli uno, viene inviata
esattamente come l'ha costruita la dApp: Permit2 è pensato attorno a
un'approvazione permanente, e il batch di uno smart account spende l'allowance
nella stessa transazione, quindi un limite inferiore a quella spesa fa fallire
l'intero batch. Dentro un batch, ogni approvazione si può limitare nelle app iOS e
Android; sul web e su desktop il batch viene segnalato in rosso ma non è ancora
modificabile. Un ultimo controllo prima dell'invio legge la calldata grezza, quindi
un'approvazione illimitata che la schermata di approvazione non ha mai mostrato non
può partire. Un'<strong>approvazione finita ma elevata</strong> (anche molto oltre
il tuo saldo) viene mostrata con un avviso. I <strong>permit firmati</strong>
(firme EIP-2612 e Permit2) non si possono limitare — la dApp invia la propria
copia — quindi si firmano così come richiesti o si rifiutano: uno illimitato in
rosso, uno limitato con un avviso. Una richiesta di concedere un
<code>setApprovalForAll</code> degli NFT su un'intera collezione non si può ancora
approvare nelle app.
</Callout>

## Quando Vela non riesce a decodificare una chiamata

Quando non esiste un descrittore ERC-7730 ma la funzione compare in un database
pubblico di selettori, Vela decodifica la chiamata in modo generico e la etichetta
come **best effort** — decodificata, ma non verificata — sotto un banner di
attenzione. Se anche questo fallisce, o se Vela riesce a decodificare solo una
parte della transazione, **non** fa finta di averla capita.

<Callout type="danger" title="Avviso esplicito di firma alla cieca">
Se una chiamata non si può decodificare, Vela mostra un chiaro avviso di firma alla
cieca invece di un riassunto falsamente rassicurante. Se riesce a risolvere solo
alcuni campi, ti dice che la vista è parziale e tiene alto il livello di rischio.
Sai sempre quanta parte di ciò che stai firmando Vela è riuscito davvero a leggere.
</Callout>

## Perché conta

Autocustodia significa che nessuno può annullare una transazione sbagliata al
posto tuo. La difesa non è un servizio clienti: è capire cosa approvi **prima** di
approvarlo. La firma leggibile è il modo in cui Vela prova a mostrartelo, e ha dei
limiti: può essere onesta solo quanto l'app che la mostra, ed è per questo che
conta una [verifica indipendente](/it/docs/clear-signing-self-host). Nel
[whitepaper](/it/docs/whitepaper) trovi dove si colloca nel modello di sicurezza di
Vela.
