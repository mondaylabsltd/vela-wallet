---
title: Firmatari e chiavi di sicurezza
description: Un wallet Vela può avere fino a sette firmatari — passkey, un dispositivo vicino o una chiave di sicurezza tipo YubiKey — e ne basta uno qualsiasi per firmare. Si scelgono alla creazione del wallet, e questa pagina spiega perché non è un limite che ci siamo dimenticati di togliere.
---

# Firmatari e chiavi di sicurezza

Un wallet Vela è un Safe, e un Safe ha dei proprietari. Il tuo può averne **fino a
sette**, con soglia **uno**: un singolo firmatario può autorizzare una transazione
da solo. Si scrive `1-of-n`.

## Cosa può essere un firmatario

Tre tipi, liberamente mescolabili:

| Metodo | Di cosa si tratta | Esempio tipico |
| --- | --- | --- |
| **Piattaforma** | L'autenticatore integrato nel dispositivo che stai usando | Face ID / Touch ID su questo telefono o portatile, sincronizzato da Portachiavi iCloud o dal Gestore delle password di Google |
| **Dispositivo vicino** | Un altro dispositivo raggiunto scansionando un codice | Il telefono che firma per il desktop, tramite il trasporto hybrid di WebAuthn |
| **Chiave di sicurezza** | Un autenticatore removibile su USB o NFC | YubiKey e altre chiavi FIDO2 |

Tutti e tre sono credenziali WebAuthn sulla curva **P-256**. Per il Safe sono
indistinguibili: ognuno è un proprietario la cui firma il verificatore WebAuthn
on-chain controlla allo stesso modo.

Una chiave di sicurezza può essere il tuo **primo** firmatario, non solo una
riserva. Se preferisci che il wallet non dipenda mai da un account Apple o Google,
è questa l'impostazione che lo consente: registra una YubiKey alla creazione e firma
con quella.

## Perché si scelgono alla creazione

È la parte che sorprende, quindi ecco il meccanismo invece di una scusa.

L'indirizzo del wallet è **derivato** dall'insieme dei suoi proprietari. Vela lo
calcola con `CREATE2` dai dati di setup del Safe — che includono la chiave pubblica
di ogni firmatario — prima che on-chain esista alcunché. È esattamente ciò che ti
permette di ricevere fondi a un indirizzo che ancora non esiste.

La conseguenza è aritmetica, non una scelta di policy: **un insieme di chiavi
diverso è un indirizzo diverso**. Aggiungere un ottavo firmatario più tardi non
estenderebbe il tuo wallet; calcolerebbe un wallet nuovo, a un indirizzo nuovo,
senza un centesimo dei tuoi soldi dentro.

Quindi «posso aggiungere una chiave dopo?» ha due risposte oneste:

- **Prima di metterci dei fondi:** sì — l'indirizzo non si è impegnato con niente,
  ricrea il wallet con le chiavi che vuoi.
- **Dopo averci messo dei fondi:** l'indirizzo è dove stanno i tuoi soldi.
  Cambiare i proprietari di un Safe già distribuito è un'operazione Safe che Vela
  oggi non espone. Pianifica l'insieme di chiavi alla creazione.

## Da cosa ti protegge davvero

**Perdere un dispositivo.** Con più di un firmatario, un telefono perso è una
scocciatura: firma un'altra chiave. Con un solo firmatario e la sincronizzazione di
sistema disattivata, un telefono perso è un wallet perso — ecco perché «la tua
passkey si sincronizza automaticamente» descrive un'impostazione che controlli tu,
non una garanzia che possiamo dare noi al posto tuo.

**Un account di piattaforma di cui non ti fidi più.** Se la tua passkey vive nel
Portachiavi iCloud o nel Gestore delle password di Google, chi controlla
quell'account può potenzialmente usarla. Una chiave di sicurezza la tieni tu e non
si sincronizza da nessuna parte.

E ciò da cui **non** ti protegge, perché `1-of-n` taglia da entrambe le parti:
aggiungere una seconda chiave aggiunge un secondo modo per *entrare*, non un
secondo lucchetto. Chiunque ottenga uno qualsiasi dei tuoi firmatari può firmare da
solo. Più chiavi significa più resistenza alla perdita e più superficie al furto: è
questo lo scambio, e la scelta è tua.

## Recuperare non è aggiungere

Sono due cose diverse, e la documentazione le tiene separate:

- [Recupero e accesso](/it/docs/recovery) — tornare a un wallet esistente su un
  nuovo dispositivo con una chiave che hai già.
- Questa pagina — decidere in anticipo quali chiavi esistono.
