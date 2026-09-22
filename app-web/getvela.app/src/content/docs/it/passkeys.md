---
title: Come funzionano le passkey
description: "Cos'è una passkey, dove sta la chiave privata per ogni tipo di chiave, perché non c'è nessun segreto da rubare con il phishing e da cosa una passkey non ti protegge."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Come funzionano le passkey

Le chiavi che controllano un wallet Vela sono **passkey**: credenziali WebAuthn
sulla curva P-256. Il tuo dispositivo o la tua chiave di sicurezza crea ciascuna
di esse, conserva la chiave privata e la usa solo dopo che hai confermato con Face
ID, l'impronta, il PIN del dispositivo, oppure un tocco e il PIN su una chiave di
sicurezza. Vela non riceve mai la chiave privata; se un gestore di password la
sincronizza, è il gestore a custodirla, cifrata, per conto tuo.

## Cos'è una passkey

Una passkey è una coppia di chiavi pubblica/privata creata per un solo sito web —
per Vela, `getvela.app`. Un'app non riceve mai la chiave privata: può solo chiedere
all'autenticatore di firmare qualcosa, e l'autenticatore prima lo chiede a te.

Dove sta la chiave privata dipende dal tipo di chiave:

| Tipo di chiave | Dove sta la chiave privata | Sincronizzata su altri dispositivi? |
| --- | --- | --- |
| **Questo dispositivo** — Face ID, Touch ID, impronta, Windows Hello | Il gestore di password della tua piattaforma (Portachiavi iCloud, Gestore delle password di Google) o un gestore di password come 1Password | Di solito sì, con crittografia end-to-end, se la sincronizzazione è attiva. Le chiavi di Windows Hello restano sul PC |
| **Un altro telefono**, collegato scansionando un codice QR | Il gestore di password di quel telefono | Come sopra |
| **Una chiave di sicurezza hardware** (YubiKey e altre chiavi FIDO2, via USB o NFC) | Dentro la chiave di sicurezza | Mai |

Un wallet Vela può usare fino a sette chiavi, di qualsiasi combinazione, scelte
quando lo crei; [firmatari e chiavi di sicurezza](/it/docs/signers) parla di questa
scelta.

## Nessun segreto da rubare con il phishing

Il phishing funziona facendoti consegnare un segreto. Una seed phrase sono dodici
parole che qualcuno può convincerti a digitare da qualche parte. Una passkey **non
ha alcun segreto che si possa digitare**: non c'è niente da rivelare, niente da
incollare, e un sito falso non può chiedertelo. E poiché una passkey è creata per
un solo sito, il browser offre una passkey di `getvela.app` solo alle pagine di
getvela.app e dei suoi sottodomini.

Questo elimina un'intera categoria di perdite — la frase di recupero rubata — che
nell'autocustodia è frequente.

## Da cosa una passkey non ti protegge

<Callout type="warning" title="Una passkey firma qualsiasi cosa tu approvi">
La richiesta del telefono o del browser dice <em>quale</em> chiave viene usata, non
<em>che cosa</em> viene firmato. Se la approvi, una passkey firma una transazione
dannosa con la stessa facilità di una legittima. Per questo Vela decodifica ogni
transazione prima che tu firmi (<a href="/it/docs/clear-signing">firma
leggibile</a>), e per questo conta la pagina che te la mostra
(<a href="/it/docs/bybit-attack">l'attacco a Bybit</a>).
</Callout>

Non protegge nemmeno da chi ha il tuo telefono sbloccato e riesce a superarne il
controllo, o da chi controlla l'account tramite cui la passkey si sincronizza.
Tieni impostato un codice di blocco del dispositivo, proteggi il tuo account Apple
o Google e valuta una chiave di sicurezza hardware che non si sincronizza da
nessuna parte.

## Com'è firmare

1. Confermi una transazione in Vela, dopo aver letto cosa fa.
2. Il dispositivo o la chiave di sicurezza chiede Face ID, l'impronta, il PIN,
   oppure un tocco più il PIN.
3. Firma, e all'app torna solo la firma.
4. L'app passa l'operazione firmata al relay, che la invia; il contratto del tuo
   wallet controlla la firma della passkey on-chain prima di fare qualsiasi cosa.

## Dove va la chiave pubblica

Le metà **pubbliche** delle tue chiavi vengono registrate in un registro pubblico
su Gnosis Chain, così un nuovo dispositivo può trovare il tuo wallet. È
l'argomento di [recupero e accesso](/it/docs/recovery).

Poi: [firmatari e chiavi di sicurezza](/it/docs/signers).
