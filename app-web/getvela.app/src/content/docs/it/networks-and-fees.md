---
title: Reti e commissioni
description: "Le 24 reti integrate in Vela, come aggiungerne un'altra, come si calcola esattamente la commissione di una transazione e chi la riceve, e cosa succede quando un relay resta senza gas."
source: b58f2cec8d4f
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Reti e commissioni

## Reti integrate

Vela integra **24 reti**, tutte mainnet:

| Rete | Gas pagato in | Rete | Gas pagato in |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (la moneta nativa) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (la moneta nativa) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (nessuna moneta nativa) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

Sulla maggior parte di queste puoi pagare la commissione anche in una stablecoin
in dollari che il relay accetta su quella rete (vedi sotto).

Il tuo wallet ha **lo stesso indirizzo su ogni rete**, perché l'indirizzo è
calcolato dalle tue chiavi, non dalla chain.

## Aggiungere un'altra rete

Puoi aggiungere qualsiasi rete EVM in **Impostazioni → Reti**, purché abbia ciò
che serve a un wallet Vela: undici contratti standard (l'EntryPoint ERC-4337 v0.7,
i contratti Safe v1.4.1, i moduli 4337 e passkey di Safe, MultiSend, Multicall3 e
due deployer deterministici) e il precompilato **EIP-7951 / RIP-7212** che verifica le firme
delle passkey all'indirizzo `0x100`. Il wallet li controlla tutti, compresa una
vera verifica di firma sul precompilato, prima di lasciarti aggiungere la rete.
Il precompilato ha due nomi: EIP-7951 su Ethereum, attivo dall'upgrade Fusaka
(dicembre 2025), e RIP-7212 sui rollup. L'interfaccia è la stessa, e il wallet
accetta entrambi.

Il precompilato è un requisito rigido. Il suo indirizzo fa parte del calcolo di
ogni indirizzo Vela, quindi non c'è un verificatore di riserva e non c'è modo di
deployarne uno in seguito. Se una chain ha il precompilato ma le manca qualche
contratto, la [configurazione della chain](/it/chain-setup) mostra cosa manca e fa
il deploy di ciò che chiunque può deployare. Una lacuna nel controllo: un wallet
con più di una chiave ha bisogno anche della factory dei firmatari passkey di Safe
su quella rete, che per ora non viene controllata; senza di essa, lì può firmare
solo la prima chiave.

## Come si paga una transazione

Vela è un wallet ERC-4337: la transazione non la trasmetti tu. L'app costruisce una
**UserOperation**, tu la firmi con una delle tue chiavi e un **relay** la invia
on-chain anticipando il gas. (ERC-4337 chiama questo ruolo bundler.) Il relay
viene rimborsato **dentro la tua operazione**: il pagamento è un trasferimento dal
tuo wallet al relay che sta nello stesso batch della tua transazione, quindi è
coperto dalla tua firma. Non c'è alcun paymaster: nessuno sponsorizza il tuo gas,
e nessuno può rifiutare la tua transazione in base a una politica di
sponsorizzazione.

### A quanto ammonta la commissione

<span id="fee"></span>

La schermata di conferma mostra un solo importo, nella moneta della commissione e
nella tua valuta di visualizzazione. Si calcola così:

- **Gas riservato dal wallet.** Il wallet simula la transazione e riserva più gas
  di quanto preveda di usarne: le stime di verifica e di esecuzione vengono
  aumentate ciascuna della metà, con dei minimi (per esempio, la verifica è di
  almeno 300.000 gas una volta che il wallet è deployato, e 2.000.000 per la
  transazione che lo deploya).
- **Prezzo del gas.** Il più alto tra la lettura del prezzo del gas della rete
  fatta dal wallet e il prezzo del relay per la velocità che hai scelto. La
  velocità predefinita è *rapida*, che il relay prezza a circa 1,8 volte la base
  fee più il doppio della priority fee.
- **Commissione = 3 × gas riservato × prezzo del gas**, con un minimo di circa
  0,01 dollari. Su Tempo il moltiplicatore è 2 e la commissione si paga in pathUSD.

La riserva è maggiorata ben oltre ciò che la transazione userà e il prezzo include
un margine, quindi la commissione è più alta di quanto la transazione costi davvero
on-chain — e ancora più alta alla tua prima transazione su una rete, che deploya
anche il tuo wallet. Il relay paga il costo reale e tiene il resto; non
viene rimborsato nulla. Sulle reti economiche sono centesimi; sulla mainnet di
Ethereum può essere una cifra significativa. Non devi mai tirare a indovinare:
l'importo esatto è nella schermata di conferma prima che tu firmi.

**A chi va.** La commissione va a chi gestisce il relay impostato nel wallet —
quello di Vela, se non lo cambi. Funziona qualsiasi istanza di vela-relay, compresa
[una che gestisci tu](/it/docs/self-hosting#relay), e il wallet usa la stessa
formula qualunque relay tu scelga.

<Callout type="info" title="Paghi quello che vedi">
L'importo della commissione e l'indirizzo a cui va fanno parte dell'operazione che
firmi. Un relay che cambiasse uno dei due invaliderebbe la tua firma, quindi paghi
esattamente l'importo mostrato — non di più, anche se il gas sale prima
dell'inclusione. Una quotazione del prezzo del gas da parte del relay superiore al
triplo della lettura del wallet viene rifiutata.
</Callout>

### Con cosa puoi pagare

- La **moneta nativa** della rete, sempre.
- Una **stablecoin in dollari** dall'elenco del relay per quella rete, quando il
  relay riesce a prezzare la moneta nativa. Le stablecoin che non possiedi sono
  nascoste.
- Su **Tempo**, che non ha una moneta nativa, solo **pathUSD**.

La moneta della commissione e la velocità (*lenta*, *standard* o *rapida*) le
scegli nella schermata di conferma e nelle Impostazioni.

### La tua prima transazione su una rete

Puoi ricevere su qualsiasi rete prima che il tuo wallet esista lì. La prima volta
che invii da una rete, quella transazione deploya anche il contratto del tuo
wallet (e un piccolo contratto firmatario per ogni chiave in più). Il gas del
deploy è compreso nella commissione di quella transazione, quindi il primo invio
su ogni rete costa più di quelli successivi.

Quando invii il **massimo** di una moneta nativa, Vela tiene da parte quanto basta
per la commissione.

## Chi gestisce il relay — e a chi va la commissione

Per impostazione predefinita ogni rete usa il **relay di Vela**, e la commissione
va a Vela. Puoi indirizzare il wallet verso un altro relay in **Impostazioni →
Avanzate → Endpoint dei servizi**; un solo indirizzo vale per tutte le reti
integrate, mentre una rete personalizzata mantiene l'indirizzo del relay con cui è
stata aggiunta. Il relay deve essere
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — quello di Vela o uno
gestito da te — perché il wallet chiede la quotazione della commissione con un
metodo specifico di Vela che i bundler generici come Pimlico o Alchemy non
implementano. La commissione la riceve chi gestisce il relay che usi; la
[guida al self-hosting](/it/docs/self-hosting#relay) spiega come gestirne uno.

Il relay riceve un'operazione già firmata. Non può cambiare il destinatario,
l'importo, la commissione o qualsiasi altra cosa. Può ritardarla o rifiutarla, e
sceglie quando finisce on-chain — quindi, per uno swap, in linea di principio
potrebbe anticiparti con una propria operazione entro il tuo slippage.

### Quando un relay resta senza gas

Un relay paga il gas dalla propria **tesoreria** su ogni rete. Se quella tesoreria
è vuota, la schermata di invio te lo dice prima che tu firmi:

- Su una rete servita dal relay di Vela, deve ricaricarla il gestore del relay
  (Vela); puoi segnalarlo. Se non puoi aspettare, puoi **facoltativamente**
  inviare tu stesso alla tesoreria un piccolo importo della moneta nativa. Quel
  contributo **non è rimborsabile** e **non** paga la tua transazione.
- Su una rete personalizzata, finanziare il relay spetta a chi lo gestisce — che
  potresti essere tu.

Non esiste un account di gas per wallet né un deposito di attivazione: una
versione precedente di Vela ne aveva uno, e non esiste più.

## Come Vela legge ogni rete

Vela legge i saldi e simula le transazioni tramite un **pool di endpoint RPC** per
ogni rete — quelli integrati, alternative pubbliche e qualsiasi chiave di un
provider o endpoint che aggiungi tu — e passa al successivo quando uno è lento o
non risponde. Puoi impostare un tuo endpoint per ogni rete in **Impostazioni →
Reti**. (L'app Android per ora usa un solo endpoint per rete, senza failover, e
l'app iPhone non permette ancora di cambiarlo.)

Poi: [come funzionano le passkey](/it/docs/passkeys).
