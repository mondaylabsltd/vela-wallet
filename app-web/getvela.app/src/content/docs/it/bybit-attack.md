---
title: L'attacco a Bybit, e la strada che ha usato
description: "A febbraio 2025 Bybit ha perso circa 1,5 miliardi di dollari. I contratti Safe non sono stati violati — l'interfaccia sì. Questa pagina spiega la strada usata e cosa, nel design di Vela, la chiude."
source: ac56b16b531f
---

# L'attacco a Bybit, e la strada che ha usato

Il 21 febbraio 2025 Bybit ha perso circa **1,5 miliardi di dollari** da un cold
wallet multisig Safe. È il furto più grande nella storia del settore, e vale la
pena leggerlo con attenzione, perché quasi tutto è stato *corretto* tranne una
cosa.

## Cos'è successo

La versione breve, dai post-mortem pubblici:

1. Un attaccante ha compromesso la **macchina di uno sviluppatore di
   `Safe{Wallet}`** e ha iniettato JavaScript malevolo nel bucket AWS S3 che
   serviva il frontend di `Safe{Wallet}`. Il codice è stato inserito il 19
   febbraio ed è scattato il 21, mirato al Safe specifico di Bybit.
2. I firmatari di Bybit hanno aperto l'interfaccia e controllato una transazione
   che sembrava ordinaria.
3. Il payload inviato davvero ai loro **wallet hardware** non era quella
   transazione. Era un `delegatecall` che sovrascriveva la `masterCopy` del proxy
   Safe — lo slot 0 — sostituendo l'intera implementazione dell'account con quella
   dell'attaccante.
4. I firmatari hanno approvato. Le firme erano valide. Il contratto ha fatto
   esattamente ciò che gli era stato detto.

L'attribuzione pubblica ha indicato attività legate alla Corea del Nord (l'FBI ha
nominato il gruppo TraderTraitor).

## Cosa *non* è stato violato

- **Non i contratti Safe.** Hanno eseguito un'istruzione firmata in modo valido.
  Nessun bug di Safe è stato sfruttato.
- **Non la crittografia.** Ogni firma era autentica.
- **Non i wallet hardware.** I dispositivi Ledger erano nel giro e hanno firmato
  lo stesso — perché un wallet hardware mostra ciò che riceve, e ciò che ha
  ricevuto era il payload malevolo. Un dispositivo che non sa tradurre un
  `delegatecall` in qualcosa che una persona possa valutare protegge la *chiave*,
  non la *decisione*.

Ciò che è stato violato è il presupposto su cui poggia ogni interfaccia di wallet:
**che la schermata che descrive una transazione e i byte che vengono firmati
siano la stessa cosa.**

## Perché è il caso generale, non un evento anomalo

La maggior parte delle firme prodotte in un wallet web poggia su quel presupposto.
L'interfaccia costruisce il payload, l'interfaccia disegna il riepilogo, e niente
di indipendente verifica che i due coincidano. Se il codice che serve
quell'interfaccia viene sostituito — da una pipeline di build compromessa, una CDN
dirottata, una dipendenza malevola, una credenziale di deploy rubata — il
riepilogo diventa quello che vuole l'attaccante, e la tua firma è vera.

È questo il rischio a cui mira il design di firma di Vela. Non il phishing. Non
una chiave trapelata. **Una schermata di firma che ti sta mentendo.**

## Cosa fa Vela al riguardo

**Firma leggibile, fino alla calldata.** Ogni transazione viene decodificata in un
intento leggibile prima che tu approvi — importo, destinatario, cosa fa davvero la
chiamata ([ERC-7730](/it/docs/clear-signing)). Una chiamata che non riusciamo a
decodificare viene **segnalata come non decodificabile**, non mostrata in
silenzio come se andasse tutto bene. Il payload di Bybit era un `delegatecall` che
sostituiva un indirizzo di implementazione: è esattamente il tipo di cosa che
dovrebbe fermare di colpo un firmatario, e nasconderlo dietro un riepilogo
rassicurante è il motivo per cui non l'ha fatto.

Due limiti da dire con precisione. Una dApp non può chiedere direttamente a Vela
un `delegatecall` — le richieste che una pagina può fare producono chiamate
normali — quindi il payload di Bybit in sé non potrebbe arrivare per quella
strada. Ma una pagina *può* chiedere una chiamata dal tuo Safe verso se stesso:
`enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`. Una
qualsiasi di queste, firmata una sola volta, consegna l'account in modo completo
quanto il payload di Bybit — un modulo abilitato può poi eseguire un
`delegatecall` per conto suo. Vela decodifica queste chiamate ma per ora non le
blocca; **rifiuta qualsiasi richiesta il cui destinatario sia l'indirizzo del tuo
stesso wallet.** E se il codice di Vela venisse sostituito, come è successo a
quello di `Safe{Wallet}`, anche la decodifica sarebbe dell'attaccante — ed è a
questo che serve il punto successivo.

**Una strada indipendente che può controllare l'interfaccia.** Vela ha costruito
una [pagina di firma](/it/docs/clear-signing-self-host) senza build e senza
dipendenze, che decodifica la richiesta ed esegue la firma WebAuthn per conto
proprio: una sola cartella di file statici che puoi leggere da cima a fondo,
ospitare tu o caricare come estensione del browser. Il suo scopo è essere una
seconda opinione che non condivide la catena di fornitura dell'app principale.
*Stato: costruita e testata; non pubblicata, e nessuna app Vela le invia ancora
richieste.* Quando la situazione cambierà, questa pagina lo dirà chiaramente.

**Nessun ruolo di amministrazione che possiamo perdere.** Gli account di Vela sono
[Safe v1.4.1 non modificati](/it/docs/account-contract), e Vela non vi detiene
alcun ruolo privilegiato — nessuna chiave di amministrazione e nessuna via di
upgrade nostra che potremmo essere costretti a usare, o che qualcuno potrebbe usare
dopo averci compromesso.
Ma bisogna essere chiari su ciò che questo *non* elimina: la primitiva usata dagli
attaccanti di Bybit — un `delegatecall` firmato da un proprietario che riscrive
l'implementazione dell'account — esiste ancora in ogni Safe, compreso quello di
Vela (le transazioni in batch di Vela usano esse stesse un `delegatecall` verso il
MultiSend di Safe). Richiede una firma valida di una delle tue chiavi. Le difese
contro il rischio di essere convinti a darla sono la decodifica descritta sopra e
la verifica indipendente.

**Una nuova conferma per ogni firma.** Ogni firma richiede la conferma della tua
chiave — Face ID, impronta, PIN, oppure un tocco e il PIN su una chiave di
sicurezza. Non esiste una chiave di sessione a lunga durata, quindi non esiste una
finestra in cui qualcosa possa firmare al posto tuo senza di te.

**Il self-hosting come ultima rete di sicurezza.** Le app e i servizi di backend
sono open source. Se non vuoi affatto fidarti della nostra pipeline di build,
compila tu l'estensione o un'app e gestisci i servizi che ti servono — la
[guida al self-hosting](/it/docs/self-hosting) ti accompagna passo per passo. Così
la nostra pipeline di build esce dalla catena di fiducia; resta il codice che
compili, di cui ti fidi comunque, quindi leggilo.

## Cosa Vela non sostiene

Il frontend di Vela potrebbe essere compromesso nello stesso modo in cui lo è
stato quello di `Safe{Wallet}`. Il nostro codice non ha avuto un audit. Dire il
contrario sarebbe esattamente il tipo di rassicurazione a cui quell'incidente
avrebbe dovuto mettere fine.

Quello che il design prova a fare è restringere la strada: rendere il payload
leggibile invece che opaco, non avere alcun ruolo di amministrazione che si possa
usare impropriamente a tuo danno, e darti un modo di verificare con qualcosa che
non siamo noi. Il riassunto onesto è che **questa classe di attacchi è mitigata
dal design, non eliminata**, e le parti che la renderebbero più solida sono
elencate, ancora incompiute, in [audit e problemi noti](/it/docs/security-audits).

## Fonti

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
