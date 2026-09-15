---
title: Come funzionano le passkey
description: Il modello di sicurezza dietro Vela — cos'è una passkey, dove vive la tua chiave e perché non c'è nulla da rubare con il phishing.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Come funzionano le passkey

Tutto il modello di sicurezza di Vela poggia su un'idea: la chiave che controlla il
tuo wallet è una **passkey**, creata dal tuo dispositivo e custodita dal sistema
operativo — nessuna app, Vela inclusa, può leggerla — e usata solo con il tuo volto
o la tua impronta.

## Cos'è davvero una passkey

Una passkey è una coppia di chiavi pubblica/privata creata dal tuo dispositivo. La
**chiave privata** è custodita dal servizio passkey del sistema operativo — di
solito il Portachiavi iCloud su Apple, il Gestore delle password di Google su
Android — cifrata end-to-end, quindi nessuna app può leggerla o copiarla. Le app
non ricevono la chiave: ricevono il permesso di *chiedere al tuo dispositivo di
firmare qualcosa* dopo che ti sei autenticato.

È la stessa tecnologia che protegge Apple Pay e lo sblocco biometrico.

<Callout type="info" title="Il punto chiave">
Un'app — Vela compresa — può richiedere una firma, ma non vede mai la tua chiave
privata. Volto o impronta autorizzano il dispositivo a firmare; la chiave resta nel
sistema operativo, cifrata end-to-end.
</Callout>

## Perché non c'è nulla da rubare con il phishing

Il phishing funziona facendoti consegnare un segreto. Con una frase seed, quel
segreto sono dodici parole che puoi digitare in una pagina falsa. Con una passkey
**non c'è nessun segreto digitabile**. Un sito truffaldino non può chiederti di
«inserire la tua passkey», perché una passkey non si inserisce: è un'operazione
hardware sbloccata dalla tua biometria.

Questo elimina il modo di gran lunga più comune in cui si perdono fondi
auto-custoditi.

## Che effetto fa firmare una transazione

1. Confermi una transazione in Vela.
2. Il dispositivo chiede Face ID / Touch ID.
3. Il dispositivo firma la transazione con la tua passkey.
4. Vela trasmette la transazione firmata alla rete.

Lo stesso gesto con cui sblocchi il telefono — perché è lo stesso meccanismo di
passkey che il tuo dispositivo usa già dappertutto.

<Callout type="warning" title="La sicurezza del dispositivo conta ancora">
Una passkey protegge benissimo da attacchi a distanza e phishing. Non protegge da
chi ha in mano il tuo dispositivo sbloccato e supera il controllo biometrico. Tieni
un codice di blocco impostato e non passare un telefono sbloccato a qualcuno di cui
non ti fidi.
</Callout>

## Dove sta il resto

La chiave **pubblica** della tua passkey viene pubblicata in un piccolo indice
on-chain, così il wallet può essere recuperato su un nuovo dispositivo. È
l'argomento della pagina successiva: [recupero e accesso](/it/docs/recovery).
