---
title: Firmatari e chiavi di sicurezza
description: "Un wallet Vela può avere fino a sette firmatari — passkey, un telefono vicino o una chiave di sicurezza tipo YubiKey — e ne basta uno qualsiasi per firmare. Si scelgono alla creazione del wallet; questa pagina spiega perché, e cosa fare se una chiave è compromessa."
source: 072891bd4768
---

# Firmatari e chiavi di sicurezza

Un wallet Vela è un Safe, e un Safe ha dei proprietari. Il tuo può averne **fino a
sette**, con soglia **uno**: un singolo firmatario può autorizzare una transazione
da solo. Si scrive `1-of-n`.

## Cosa può essere un firmatario

Tre tipi, che puoi combinare liberamente:

| Metodo | Di cosa si tratta | Esempio tipico |
| --- | --- | --- |
| **Piattaforma** | L'autenticatore integrato nel dispositivo che stai usando | Face ID / Touch ID su questo telefono o portatile, sincronizzato dal Portachiavi iCloud o dal Gestore delle password di Google |
| **Dispositivo vicino** | Un altro dispositivo, collegato scansionando un codice | Il telefono che firma per il computer, tramite il trasporto ibrido di WebAuthn |
| **Chiave di sicurezza** | Un autenticatore rimovibile, USB o NFC | YubiKey e altre chiavi FIDO2 |

Tutti e tre sono credenziali WebAuthn sulla curva **P-256**. Per il Safe sono
indistinguibili: ognuno è un proprietario la cui firma il modulo passkey di Safe
controlla on-chain allo stesso modo. (La prima chiave viene verificata dal
firmatario condiviso di Safe; ogni chiave aggiuntiva dal proprio piccolo contratto
firmatario, creato dalla factory di Safe la prima volta che il wallet viene
deployato su una chain.)

Quali tipi può usare ogni app:

| App | Questo dispositivo | Telefono vicino (QR) | Chiave di sicurezza |
| --- | --- | --- | --- |
| Wallet web, estensione del browser | Sì | Sì | USB o NFC, tramite il browser |
| Desktop (macOS, Windows, Linux) | macOS e Windows | Sì | USB |
| Android | Sì (con i servizi Google Play) | Sì | USB |
| iOS | Sì | Sì | YubiKey USB-C o Lightning, firmware 5.8 o successivo |

L'opzione «questo dispositivo» dell'app desktop (Touch ID, Windows Hello) e il suo
supporto a Windows in generale sono recenti e meno collaudati delle altre strade;
lì, per ora, la scelta affidabile è un telefono o una chiave di sicurezza.

Una chiave di sicurezza può essere il tuo **primo** firmatario, non solo una
riserva. Se preferisci che il wallet non dipenda mai da un account Apple o Google,
crealo con **due** chiavi di sicurezza e tienine una in un posto sicuro. (Un
wallet la cui unica chiave non è sincronizzata da nessuna parte non si può creare:
l'app chiede una seconda chiave, perché perdere quel solo dispositivo vorrebbe dire
perdere il wallet.)

## Perché si scelgono alla creazione

È la parte che sorprende, quindi ecco il meccanismo invece di una scusa.

L'indirizzo del wallet è **derivato** dall'insieme dei suoi proprietari. Vela lo
calcola con `CREATE2` dai dati di setup del Safe — che includono la chiave pubblica
di ogni firmatario — prima di qualsiasi deploy on-chain. È ciò che ti permette di
ricevere fondi a un indirizzo che ancora non esiste.

Per l'indirizzo, la conseguenza è aritmetica: **un insieme di chiavi diverso è un
indirizzo diverso**. Aggiungere un firmatario più tardi non estenderebbe il tuo
wallet; ne calcolerebbe uno nuovo, a un indirizzo nuovo, senza un centesimo dei
tuoi soldi dentro.

Quindi alla domanda «posso aggiungere una chiave dopo?» ci sono due risposte
oneste:

- **Prima di metterci dei fondi**: sì — l'indirizzo non è ancora legato a niente,
  quindi ricrea il wallet con le chiavi che vuoi.
- **Dopo averci messo dei fondi**: l'indirizzo è dove stanno i tuoi soldi. Safe di
  per sé può cambiare i proprietari su una chain dove il wallet è già deployato —
  ma su ogni chain dove non lo è ancora, lo stesso indirizzo corrisponde ancora
  alle chiavi originali, quindi gli insiemi di proprietari finirebbero per
  divergere da una chain all'altra. Tenerli allineati tra le chain è possibile —
  alcuni smart wallet lo fanno — ma Vela non l'ha realizzato, quindi non offre il
  cambio dei proprietari. Pianifica l'insieme di chiavi alla creazione.

## Da cosa ti protegge davvero

**Perdere un dispositivo.** Con più di un firmatario, un telefono perso è una
seccatura: firma un'altra chiave. Con un solo firmatario e la sincronizzazione di
sistema disattivata, un telefono perso è un wallet perso — ecco perché «la tua
passkey si sincronizza automaticamente» descrive un'impostazione che controlli tu,
non una garanzia che possiamo darti noi.

**Un account di piattaforma di cui non ti fidi più.** Se la tua passkey sta nel
Portachiavi iCloud o nel Gestore delle password di Google, chi controlla
quell'account può potenzialmente usarla. Una chiave di sicurezza la tieni tu e non
si sincronizza da nessuna parte.

E ciò da cui **non** ti protegge, perché `1-of-n` è un'arma a doppio taglio:
aggiungere una seconda chiave aggiunge un secondo modo per *entrare*, non un
secondo lucchetto. Chiunque ottenga uno qualsiasi dei tuoi firmatari può firmare
da solo. Più chiavi significa più resistenza alla perdita e più superficie esposta
al furto: è questo lo scambio, e la scelta spetta a te.

## Se una chiave potrebbe essere compromessa

Una chiave non si può rimuovere. Se una delle tue chiavi potrebbe essere finita
nelle mani di qualcun altro — un telefono sbloccato che è sparito, un codice che
qualcuno ha visto, un account Apple o Google che non controlli più — **sposta tutto
in un nuovo wallet** creato con chiavi di cui ti fidi. Il vecchio indirizzo resta
spendibile da quella chiave su ogni rete, compresi i fondi che chiunque vi invii in
futuro.

## Recuperare non è aggiungere

Sono due cose diverse, e la documentazione le tiene separate:

- [Recupero e accesso](/it/docs/recovery) — tornare a un wallet esistente su un
  nuovo dispositivo con una chiave che hai già.
- Questa pagina — decidere in anticipo quali chiavi esistono.
