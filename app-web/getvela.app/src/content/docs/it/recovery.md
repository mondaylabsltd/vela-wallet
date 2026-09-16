---
title: Recupero e accesso
description: Come Vela ti fa recuperare il wallet su un nuovo dispositivo senza frase seed — e i limiti onesti di questo modello.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recupero e accesso

La parte più difficile di un wallet senza frase seed è proprio il recupero: se non
ci sono dodici parole, come torni dentro da un telefono nuovo? Ecco esattamente
come lo gestisce Vela.

## Come funziona

Quando crei un wallet, due cose vengono pubblicate nell'**indice passkey** di Vela:

- La **chiave pubblica** della tua passkey (mai quella privata).
- Il **nome** che hai scelto per il wallet.

La chiave pubblica è memorizzata sulla blockchain Gnosis tramite un contratto, ed è
quindi leggibile pubblicamente e non dipende dal fatto che i server di Vela restino
online.

La tua chiave **privata**, intanto, è una passkey sincronizzata dal portachiavi
della tua piattaforma: **Portachiavi iCloud** sui dispositivi Apple, **Gestore
delle password di Google** su Android.

Per accedere da un nuovo dispositivo:

1. Accedi allo stesso account iCloud o Google, con la sincronizzazione del
   portachiavi attiva.
2. Apri Vela e scegli di accedere.
3. Autenticati con la passkey. La piattaforma fornisce la passkey sincronizzata;
   l'indice fornisce l'account corrispondente. Il tuo wallet è tornato.

L'indice è una cache, non un punto singolo di rottura. Se dovesse risultare
irraggiungibile e il tuo account non fosse nella memoria locale, Vela può
ricostruire la tua chiave pubblica sul dispositivo a partire da due firme passkey e
riderivarne l'indirizzo del wallet — senza alcun server.

<Callout type="info" title="Perché dividerlo così">
La chiave pubblica nell'indice on-chain permette a chiunque (anche a
un'installazione appena fatta) di trovare il tuo account. La chiave privata,
sincronizzata dal portachiavi della piattaforma di cui ti fidi, è ciò che autorizza
davvero le transazioni. Tutto ciò che sta nell'indice è dato pubblico, e niente lì
dentro può spostare i tuoi fondi: possono farlo solo le firme della tua passkey.
</Callout>

## I limiti, detti onestamente

Auto-custodia significa che la responsabilità è davvero tua. Ecco cosa capire.

<Callout type="warning" title="Il tuo recupero dipende dal portachiavi della piattaforma">
L'accesso multi-dispositivo di Vela si appoggia alla sincronizzazione della passkey
tramite Portachiavi iCloud o Gestore delle password di Google. Tieni quell'account
sicuro e le sue opzioni di recupero aggiornate. Se perdi <strong>sia</strong> i tuoi
dispositivi <strong>sia</strong> il portachiavi dell'account della piattaforma, Vela
non può rigenerare la tua chiave privata: per come è progettato, non l'abbiamo mai
avuta.
</Callout>

Consigli pratici:

- **Tieni attiva la sincronizzazione del portachiavi.** È ciò che porta la passkey
  da un dispositivo all'altro.
- **Metti in sicurezza l'account Apple / Google** con una password forte e i suoi
  metodi di recupero. Quell'account fa ormai parte della sicurezza del tuo wallet.
- **Tieni più di un dispositivo collegato** dove puoi, così un telefono perso resta
  una scocciatura e non una crisi.

## Cosa può e cosa non può fare Vela

- **Può:** aiutarti a ritrovare il tuo account tramite l'indice pubblico.
- **Non può:** spostare i tuoi fondi, congelare il wallet o recuperare una chiave
  privata. Vela non l'ha mai avuta. È tutto il senso dell'auto-custodia — e lo
  scambio che accetti in cambio.
