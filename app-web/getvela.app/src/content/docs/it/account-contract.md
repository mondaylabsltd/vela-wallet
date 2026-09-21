---
title: Il contratto dell'account
description: "Il tuo wallet Vela è un Safe v1.4.1 non modificato. Nessun contratto sulla strada verso i tuoi soldi è stato scritto da Vela — ecco esattamente quali contratti sono, cosa ti danno e cosa costano."
source: 588a6ba6e672
---

# Il contratto dell'account

Il tuo wallet non è una struttura dati privata di un'app. È uno smart account
**Safe v1.4.1** — il contratto usato da molte grandi tesorerie on-chain —
deployato esattamente come Safe lo pubblica, senza alcuna modifica.

## Sulla strada non c'è niente di nostro

Ogni contratto che può toccare i tuoi soldi è stato scritto da Safe o dagli autori
di ERC-4337:

| Contratto | Ruolo nel tuo wallet | Scritto da |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, tramite un proxy) | L'account stesso: proprietari, soglia, esecuzione | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Permette all'EntryPoint di operare il Safe; è anche il suo fallback handler | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Verifica le firme P-256 della prima chiave | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) e i firmatari che crea | Un piccolo contratto firmatario per ogni chiave aggiuntiva | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Esegue la tua operazione firmata | Gli autori di ERC-4337 |

I contratti di Vela non sono in questo elenco: il **registro delle chiavi
pubbliche** che registra le chiavi di ogni wallet così che un nuovo dispositivo
possa trovarlo ([recupero](/it/docs/recovery)), il registro di dominio che lo
affianca e il vecchio indice che hanno sostituito. Non custodiscono fondi e non
hanno alcun ruolo nel tuo Safe.

Il repository del wallet non contiene alcun codice Solidity — puoi verificarlo con
un comando:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # non stampa nulla
```

Vela non ha alcun ruolo privilegiato sul tuo account: nessuna chiave di
amministrazione, nessuna via di upgrade, nessun modulo che possa aggiungere. Solo
le tue chiavi possono modificare il tuo Safe.

## Perché «non modificato» è la parola che conta

Molti wallet sono costruiti su «un Safe», su un fork di Safe o su un account
ispirato a Safe. La differenza conta per tre motivi.

**Gli audit valgono per ciò che stai usando davvero.** Gli audit di Safe coprono
queste release, o release precedenti da cui differiscono solo per piccole
modifiche documentate (la [pagina sugli audit](/it/docs/security-audits) ha i
dettagli). Gli audit di un fork coprono il codice prima del fork; la modifica è
proprio la parte che nessuno ha controllato.

**L'ecosistema tratta il tuo account come un Safe, perché lo è.** Gli explorer lo
decodificano, e gli strumenti di Safe possono leggerlo e costruire transazioni per
lui. Per *firmare* quelle transazioni, però, un programma deve poter chiedere alla
tua chiave una firma per `getvela.app`, il dominio a cui appartengono le tue
passkey — quindi l'app web di Safe, servita da un altro dominio, non può firmare
al posto tuo. La
[guida al self-hosting](/it/docs/self-hosting#if-getvela-app-disappears) elenca
cosa invece può farlo.

**La superficie d'attacco è sotto gli occhi di molti altri.** Un contratto di
account su misura lo sorveglia soprattutto chi l'ha scritto. I contratti core di
Safe li sorveglia chiunque tenga soldi in un Safe; i moduli 4337 e passkey hanno un
pubblico più ristretto, ma reale.

## Cosa costa

Essere standard non è gratis:

- **Gas.** La tua firma viene verificata on-chain e la transazione passa per
  l'EntryPoint. Un semplice invio da un wallet Vela già deployato ha usato circa
  140.000–170.000 gas on-chain nelle nostre misurazioni su Gnosis (settembre
  2026); un semplice trasferimento di ETH da un account normale ne usa 21.000. Al
  gas si aggiunge la commissione del relay — vedi
  [reti e commissioni](/it/docs/networks-and-fees).
- **L'account va deployato.** Il tuo indirizzo è calcolato con `CREATE2` prima che
  on-chain esista qualsiasi cosa, quindi puoi ricevere subito; la tua prima
  transazione in uscita su ogni rete paga il deploy del contratto.
- **Non tutte le chain sono idonee.** Le firme delle passkey vengono verificate con
  il precompilato **RIP-7212**, e il suo indirizzo fa parte dei dati di setup di
  ogni wallet, quindi una rete che non ce l'ha non può far girare Vela in alcun
  modo.
- **Il rischio di Safe ora è anche il tuo.** Fidarsi di un contratto molto usato
  resta pur sempre fidarsi di un contratto. Vela non ha aggiunto sulla strada verso
  i tuoi soldi un secondo contratto suo di cui dovresti fidarti.

## Cosa ha avuto un audit e cosa no

I contratti di Safe, i suoi moduli 4337 e passkey e l'EntryPoint v0.7 hanno audit
pubblici di terze parti. **Il codice di Vela — le app, i servizi di backend e il
contratto di registro — non ha avuto un audit di terze parti, e non ce n'è nessuno
in programma**; è un obiettivo per quando il progetto potrà finanziarlo, non un
impegno con una data. Ogni contratto, il relativo report di audit e i problemi che
seguiamo sono in [audit e problemi noti](/it/docs/security-audits).

## Verifica tu stesso

Il tuo account è on-chain. Una volta deployato, apri il tuo indirizzo in un block
explorer: è un proxy Safe la cui implementazione è il deploy canonico di SafeL2
v1.4.1 di Safe, su ogni rete.

Poi: [audit e problemi noti](/it/docs/security-audits).
