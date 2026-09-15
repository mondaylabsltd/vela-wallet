---
title: L'attacco a Bybit
description: A febbraio 2025 Bybit ha perso circa 1,5 miliardi di dollari. I contratti Safe non si sono rotti — si è rotta l'interfaccia. Questa pagina spiega il percorso usato e cosa lo chiude nel design di Vela.
---

# L'attacco a Bybit

Il 21 febbraio 2025 Bybit ha perso circa **1,5 miliardi di dollari** da un cold
wallet multisig Safe. È il furto più grande nella storia del settore, e vale la
pena leggerlo con attenzione, perché quasi tutto è stato *corretto* tranne una
cosa.

## Cos'è successo

La versione breve, dai post-mortem pubblici:

1. Un attaccante ha compromesso la **macchina di uno sviluppatore di
   `Safe{Wallet}`** e iniettato JavaScript malevolo nel bucket AWS S3 che serviva
   il frontend di `Safe{Wallet}`. Il codice è entrato il 19 febbraio ed è scattato
   il 21, mirato al Safe specifico di Bybit.
2. I firmatari di Bybit hanno aperto l'interfaccia e revisionato una transazione
   che sembrava ordinaria.
3. Il payload arrivato davvero ai loro **wallet hardware** non era quella
   transazione. Era un `delegatecall` che sovrascriveva la `masterCopy` del proxy
   Safe — lo slot 0 — sostituendo l'intera implementazione dell'account con quella
   dell'attaccante.
4. I firmatari hanno approvato. Le firme erano valide. Il contratto ha fatto
   esattamente ciò che gli è stato detto.

L'attribuzione pubblica ha indicato attività collegata alla Corea del Nord (l'FBI
ha nominato il gruppo TraderTraitor).

## Cosa *non* si è rotto

- **Non i contratti Safe.** Hanno eseguito un'istruzione validamente firmata.
  Nessun bug di Safe è stato sfruttato.
- **Non la crittografia.** Ogni firma era autentica.
- **Non i wallet hardware.** I dispositivi Ledger erano nel giro e hanno firmato
  lo stesso — perché un wallet hardware mostra ciò che gli viene dato, e ciò che
  gli è stato dato era il payload malevolo. Un dispositivo che non sa tradurre un
  `delegatecall` in qualcosa che un umano possa valutare protegge la *chiave*, non
  la *decisione*.

Ciò che si è rotto è l'assunto sotto ogni interfaccia di wallet: **che la schermata
che descrive una transazione e i byte che vengono firmati siano la stessa cosa.**

## Perché è il caso generale, non un evento anomalo

Ogni firma che hai mai prodotto in un wallet web poggiava su quell'assunto.
L'interfaccia costruisce il payload, l'interfaccia disegna il riassunto, e niente di
indipendente verifica che i due coincidano. Se il codice che serve quell'interfaccia
viene sostituito — pipeline di build compromessa, CDN dirottata, dipendenza
malevola, credenziale di deploy rubata — il riassunto diventa quello che vuole
l'attaccante, e la tua firma è vera.

È questo il rischio a cui punta il design di firma di Vela. Non il phishing. Non
una chiave trapelata. **Una schermata di firma che ti sta mentendo.**

## Cosa fa Vela al riguardo

**Firma leggibile, fino alla calldata.** Ogni transazione viene tradotta in
intenzione leggibile prima che tu approvi — importo, destinatario, cosa fa davvero
la chiamata ([ERC-7730](/it/docs/clear-signing)). Una chiamata che non riusciamo a
decodificare viene **segnalata come non decodificabile**, non disegnata in silenzio
come se andasse tutto bene. Il payload di Bybit era un `delegatecall` che
sostituiva un indirizzo di implementazione: è esattamente il tipo di cosa che deve
fermare di colpo un firmatario, e nasconderla dietro un riassunto gentile è il
motivo per cui non è successo.

**Un percorso indipendente capace di controllare l'interfaccia.** Vela sta
costruendo una pagina di firma senza build e senza dipendenze, che disegna
l'intenzione ed esegue la firma WebAuthn per conto proprio: una sola cartella di
file statici che puoi leggere da cima a fondo, servire da te o far girare come
estensione del browser. Il suo scopo è essere una seconda opinione che non
condivide la catena di fornitura dell'app principale. *Stato: costruita e testata,
non ancora distribuita.* Quando uscirà sarà opzionale, e questa pagina lo dirà
chiaramente appena cambia.

**Nessun contratto che possiamo aggiornare.** Il payload di Bybit ha funzionato
sostituendo l'implementazione dell'account. Gli account di Vela sono
[Safe v1.4.1 non modificati](/it/docs/account-contract) e Vela non vi detiene alcun
ruolo privilegiato: nessuna chiave di amministrazione, nessun percorso di upgrade
che possano costringerci o compromettere a usare.

**Una verifica biometrica nuova per ogni firma.** Non esiste una chiave di sessione
a lunga vita, quindi non esiste una finestra in cui qualcosa possa firmare al posto
tuo senza di te.

**L'auto-hosting come rete di sicurezza.** L'app e ogni servizio backend sono open
source. Se non vuoi fidarti affatto della nostra pipeline di build, falla girare
tu: è l'unica risposta a questa classe di attacchi che non richiede di fidarsi di
qualcuno.

## Cosa Vela non sostiene

Il frontend di Vela potrebbe essere compromesso allo stesso modo di quello di
`Safe{Wallet}`. Il nostro codice non è auditato. Dire altro sarebbe esattamente il
tipo di rassicurazione a cui quell'incidente avrebbe dovuto mettere fine.

Quello che il design prova a fare è restringere il percorso: rendere il payload
leggibile invece che opaco, togliere la primitiva di upgrade su cui l'attacco si è
appoggiato, e darti un modo di verificare con qualcosa che non siamo noi. Il
riassunto onesto è che **questa classe di attacchi è mitigata dal design, non
eliminata** — e le parti che la renderebbero più solida sono elencate, incompiute,
in [audit e problemi noti](/it/docs/security-audits).

## Fonti

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
