---
title: Il contratto dell'account
description: Il tuo wallet Vela è un Safe v1.4.1 non modificato. Nel percorso dei contratti non c'è nulla scritto da noi — ecco cosa ti dà e cosa costa.
---

# Il contratto dell'account

Il tuo wallet non è una struttura dati privata di un'app. È un account intelligente
**Safe v1.4.1** — lo stesso contratto usato per custodire tesorerie ben più grandi
di qualsiasi cosa Vela vedrà mai — distribuito esattamente come Safe lo pubblica,
senza modifiche.

La frase è breve, le conseguenze no: questa pagina le mette in chiaro.

## Nel percorso non c'è nulla di nostro

Fra te e i tuoi soldi ci sono quattro contratti. Vela non ne ha scritto nessuno:

| Contratto | Chi l'ha scritto |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (l'account stesso, un proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (verifica la tua chiave P-256) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | Gli autori di ERC-4337 |

Non esiste un contratto Vela. Il repository non contiene alcun Solidity —
verificabile con un comando:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # non stampa nulla
```

Quando Vela aggiunge una rete, distribuisce **quei** contratti ai loro indirizzi
canonici. Non distribuisce un contratto di propria progettazione e non detiene
alcun ruolo privilegiato sul tuo: niente chiave di amministrazione, niente percorso
di upgrade, nessun modulo che possiamo aggiungere.

## Perché «non modificato» è la parola che conta

Molti wallet si dicono costruiti su «un Safe», su «un fork di Safe» o su «un account
ispirato a Safe». Un fork è un contratto nuovo con una reputazione vecchia. In
pratica le differenze sono queste:

**Gli audit valgono per ciò che stai usando davvero.** I report di audit di Safe
coprono il bytecode di queste esatte release. Gli audit di un fork coprono il
codice prima del fork. Se un wallet ha modificato il contratto dell'account, ogni
audit che cita è l'audit di qualcos'altro — e la modifica è proprio la parte che
nessuno ha guardato.

**L'ecosistema tratta il tuo account come un Safe, perché lo è.** Gli explorer lo
decodificano. Gli strumenti di transazione di Safe lo capiscono. Se Vela sparisse
domani, il tuo wallet non è un formato orfano: è l'account intelligente con più
strumenti attorno su Ethereum, e qualsiasi interfaccia compatibile con Safe può
guidarlo. È ciò che rende
[«se Vela sparisce, il tuo wallet no»](/it/docs/why-vela) un'affermazione sui
contratti e non sulle nostre intenzioni.

**La superficie d'attacco è quella che stanno guardando tutti.** Un contratto di
account su misura lo guarda solo chi l'ha scritto. Questo lo guardano tutti quelli
che tengono soldi in un Safe.

## Cosa costa

Essere standard non è gratis, e i compromessi sono reali:

- **Gas.** Un account intelligente verifica una firma on-chain. Aspettati circa
  1,5–3 volte il gas di un semplice trasferimento EOA, a seconda della chain. Vedi
  [reti e commissioni](/it/docs/networks-and-fees).
- **L'account va distribuito.** Il tuo indirizzo è calcolato con `CREATE2` prima
  che on-chain esista qualcosa, quindi puoi ricevere subito, ma la prima
  transazione in uscita paga la distribuzione del contratto.
- **Non tutte le chain sono idonee.** Il firmatario WebAuthn verifica una firma
  P-256 on-chain, il che richiede il precompilato **RIP-7212**. Vela rifiuta di
  abilitare una rete che non ce l'ha, invece di ripiegare su un verificatore più
  debole.
- **Il rischio di Safe ora è anche il tuo.** Fidarsi di un contratto molto usato
  resta pur sempre fidarsi di un contratto. Quello che Vela può dire è che non ha
  aggiunto sopra una seconda cosa di cui fidarti.

## Cos'è auditato e cosa no

I contratti di Safe e il modulo firmatario WebAuthn sono auditati da terze parti, e
quei report sono pubblici. **Il codice applicativo di Vela non ha avuto un audit
indipendente**, e nessuno è in programma: è un obiettivo per quando il progetto
potrà finanziarlo, non un impegno con una data. Ogni contratto da cui Vela dipende,
il suo report di audit e i problemi che seguiamo sono elencati in
[audit e problemi noti](/it/docs/security-audits).

## Verifica tu stesso

Il tuo account è on-chain. Aprilo in un block explorer e leggi l'indirizzo
dell'implementazione: sarà il deployment canonico di Safe alla v1.4.1, byte per
byte, su ogni rete che Vela supporta.
