---
title: Reti e commissioni
description: Le 12 reti supportate da Vela, come funzionano le commissioni di gas con l'astrazione dell'account, chi gestisce il relay e incassa le commissioni, quando paghi tu l'attivazione dell'account di gas e come Vela sceglie gli endpoint RPC.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Reti e commissioni

## Reti supportate

Vela include **12 reti EVM**:

| Rete | Token nativo delle commissioni |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Il tuo wallet ha **lo stesso indirizzo su tutte**, quindi c'è un solo indirizzo da
condividere ovunque.

Puoi anche **aggiungere reti personalizzate** (Impostazioni → Reti). Poiché Vela è
un wallet ad account intelligente, una rete deve fornire i contratti su cui Vela si
appoggia: l'EntryPoint ERC-4337, i contratti Safe e il precompilato di firma
**P-256 (RIP-7212)** che verifica la tua passkey on-chain. Vela lo controlla
automaticamente prima di lasciarti aggiungere una rete.

<Callout type="info" title="Perché Gnosis compare così spesso">
Oltre a essere una delle 12 reti, Gnosis Chain ospita l'**indice passkey** di Vela:
il contratto che conserva la tua chiave pubblica e il nome dell'account per il
recupero multi-dispositivo. È una cosa distinta dalla rete su cui fai le
transazioni.
</Callout>

## Come funzionano le commissioni (astrazione dell'account)

Vela usa l'**astrazione dell'account ERC-4337**: una transazione non la trasmetti
tu direttamente, è una **UserOperation** consegnata a un **relay**, che la sottopone
on-chain e viene rimborsato del gas. (La specifica ERC-4337 chiama questo ruolo
*bundler*. Quello di Vela si chiama relay perché fa più che impacchettare: quota le
commissioni in banda e gestisce il protocollo dell'account di gas descritto sotto,
nessuno dei due parte dello standard.) Ne discendono alcune cose:

- **Il gas si paga dal saldo del tuo wallet** — nel token nativo della rete (ETH,
  BNB, xDAI…) per impostazione predefinita, o in una stablecoin supportata dove il
  relay ne offre una; l'asset con cui paghi le commissioni lo scegli nella schermata
  di conferma. Tempo non ha moneta nativa, quindi lì il gas è sempre regolato in
  stablecoin USD. Non c'è alcun **paymaster** ERC-4337 che sponsorizzi — o
  condizioni — ogni transazione. (Vela può farsi carico dell'unica _attivazione
  dell'account di gas_ per i nuovi utenti: è un'altra cosa, spiegata sotto.)
- **È il relay a quotare il prezzo del gas** — è l'unica fonte di verità, e il
  wallet mostra quella quotazione e firma esattamente ciò che mostra. Non c'è un
  selettore di velocità: ogni transazione viene inviata ad alta priorità.
- Il totale è il **costo di rete più la commissione di servizio del relay**, con un
  piccolo minimo sulle transazioni molto economiche. La quotazione del relay è il
  prezzo: non c'è un listino separato da consultare. Una parte va ai validatori
  della chain; il resto paga il relay che anticipa il gas e gestisce
  l'infrastruttura.
- La schermata di conferma mostra la **commissione stimata** nell'asset delle
  commissioni e nella tua valuta di visualizzazione prima che tu firmi. L'importo
  quotato e il suo destinatario fanno parte di ciò che firmi, quindi il relay riceve
  esattamente quanto mostrato: un numero cambiato invaliderebbe la tua firma.

## Chi gestisce il relay — e a chi vanno le commissioni

Ogni rete punta a un relay. Per impostazione predefinita è **il relay di Vela**, e
puoi sostituire l'endpoint in _Impostazioni → Avanzate → Endpoint dei servizi_. Un
endpoint vale per tutte le reti integrate; una rete personalizzata mantiene l'URL
del relay che le hai dato quando l'hai aggiunta.

Un'avvertenza onesta sulla compatibilità: l'app quota le commissioni con un metodo
RPC specifico di Vela (`vela_getInBandGasQuote`), e senza quello il flusso di invio
si blocca. L'endpoint a cui punti deve quindi far girare
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — l'istanza di Vela o una
che ospiti tu. Un bundler ERC-4337 generico come **Pimlico** o **Alchemy** non
implementa quel metodo, quindi nella release attuale non funzionerà da capo a fondo.

Chi gestisce il relay per una rete **incassa le commissioni di quella rete**: il
ricarico del relay su ogni transazione e il deposito di attivazione dell'account di
gas. Fai girare il tuo vela-relay e quelle commissioni finanziano la tua
infrastruttura invece di quella di Vela; sul traffico che indirizzi altrove Vela non
prende nulla.

<Callout type="warning" title="L'account di gas fa parte del protocollo vela-relay">
Il passaggio di **attivazione dell'account di gas** finanzia un account di relay
dedicato al tuo wallet su ogni rete. Se punti l'endpoint a un vela-relay ospitato da
te, il deposito finanzia l'account del tuo relay, non quello di Vela.
</Callout>

### Attivare l'account di gas (Vela Relay)

Sul relay di Vela, la tua prima transazione su ogni rete **attiva un account di gas
dedicato**. L'app chiede prima alla tesoreria del relay di finanziarlo per te:
succede in silenzio dentro il flusso di invio, e un wallet sponsorizzato non vede
mai una schermata di finanziamento. Solo quando la sponsorizzazione viene rifiutata
l'app mostra una richiesta di ricarica: invii un piccolo importo del token nativo
all'indirizzo dell'account di gas che ti mostra, e ti dice perché la
sponsorizzazione non era disponibile.

**La commissione di attivazione la paghi tu** ogni volta che non viene offerta una
sponsorizzazione gratuita, cioè quando:

- **La tesoreria di Vela per quella rete è vuota o scarsa** — il fondo gratuito su
  quella chain è temporaneamente esaurito.
- **Hai esaurito la quota gratuita** — la sponsorizzazione ha un tetto per wallet,
  oltre le prime volte è a carico tuo.
- **Il relay di Vela non finanzia affatto quella rete** — per esempio **reti
  personalizzate o di test aggiunte da te**, per le quali Vela non tiene alcuna
  tesoreria. (Indirizzale al tuo relay se preferisci saltare del tutto
  l'attivazione.)

Il deposito di attivazione è **non rimborsabile**: è il saldo iniziale dell'account
del relay e si ricarica nel tempo con i rimborsi del gas, ma può comunque esaurirsi
e richiedere una **riattivazione** più avanti. Anche un aggiornamento del servizio
può cambiare l'indirizzo del relay, e in quel caso serve una nuova attivazione.

La commissione esce dal tuo saldo nell'**asset delle commissioni** che hai scelto —
il token nativo per impostazione predefinita. Se un invio è bloccato per il gas
significa che il saldo in quell'asset non copre la commissione; dove il relay offre
gas in stablecoin, cambiare asset nella schermata di conferma può sbloccarlo.

Quando invii l'importo **massimo** di un token nativo, Vela mette
automaticamente da parte abbastanza per il gas, così la transazione non fallisce.

## Come Vela parla con ogni rete

Vela legge i saldi e invia le transazioni attraverso un **pool di endpoint RPC**,
non un singolo fornitore. Raccoglie endpoint da più fonti, li valuta per latenza e
affidabilità e **fa failover automatico** quando uno è lento o giù — mettendo in
panchina temporaneamente quelli scadenti — così un singolo nodo ballerino non manda
mai offline l'app.

Poi: [come funzionano le passkey](/it/docs/passkeys).
