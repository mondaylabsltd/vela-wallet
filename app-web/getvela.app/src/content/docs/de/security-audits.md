---
title: Audits und bekannte Probleme
description: "Jeder Vertrag, von dem Vela abhängt, wer welche Version auditiert hat, ob die auditierte Version die bereitgestellte ist, die offenen Befunde, die wir beobachten, und was überhaupt nicht auditiert wurde."
source: c4c50ad89f2f
---

„Auditiert“ ist eine Aussage über bestimmten Code in einer bestimmten Version, deshalb
nennt diese Seite die Berichte, die Commits und die bereitgestellten Adressen – und
listet auf, was **nicht** auditiert ist, was genauso wichtig ist.

Zuletzt geprüft: 22. September 2026. Wenn du einen Fehler findest, sag es uns, und wir
korrigieren ihn.

## Der Weg zu deinem Geld

Jeder Vertrag, der dein Geld berühren kann, ist eine kanonische Bereitstellung von
Code Dritter mit veröffentlichten Prüfberichten.

### Safe v1.4.1 – das Konto selbst

Deine Wallet ist ein [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)-Proxy,
der den SafeL2-Singleton und die SafeProxyFactory nutzt. Batches laufen über
MultiSend.

[Ackee Blockchain hat Safe v1.4.0 auditiert](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(Abschlussbericht 16. März 2023, Fix-Review 28. März): 11 Befunde, keiner kritisch
oder hoch; die beiden mittleren Befunde wurden zur Kenntnis genommen statt behoben.
Geprüft wurden SafeL2, SafeProxyFactory, CompatibilityFallbackHandler,
MultiSendCallOnly und SignMessageLib. v1.4.1 unterscheidet sich von v1.4.0 in einer
funktionalen Zeile, einem ERC-4337-Kompatibilitätsfix im Modul-Setup
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); Safe hat
Ackee konsultiert und kam zu dem Schluss, dass kein erneutes Audit nötig war.
MultiSend ist in seiner Logik seit v1.3.0 unverändert, und v1.3.0 hat
[G0 Group auditiert](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Alle Adressen stimmen mit [safe-deployments](https://github.com/safe-global/safe-deployments)
überein. Die Kernverträge fallen unter das
[Bug-Bounty der Safe Foundation](https://docs.safefoundation.org/security/bug-bounty),
dessen höchste Stufe bis zu 1.000.000 US-Dollar zahlt.

Der Bybit-Vorfall von 2025 ist kein Vertragsbefund: Angreifer manipulierten das
JavaScript, das an die Web-Oberfläche von Safe ausgeliefert wurde, und Safes
[forensische Stellungnahme](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
fand keine Schwachstelle in den Verträgen. [Unsere Seite dazu](/de/docs/bybit-attack)
erklärt, warum dieselbe Angriffsklasse jede Wallet-Oberfläche betrifft, auch unsere.

### Safe4337Module v0.3.0 – der ERC-4337-Adapter

Bereitgestellt unter `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (exakte
Übereinstimmung auf Sourcify) und zugleich als Fallback-Handler deines Safe gesetzt.
Dreimal geprüft –
[Berichte hier](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, Abschlussbericht März 2024: eine Warnung (Nutzung des
  Compiler-Optimierers) zur Kenntnis genommen, nichts Höheres offen.
- **Certora**, August 2026: ein **mittlerer** Befund, zur Kenntnis genommen und in
  v0.3.0 **nicht behoben** – *Änderungen der Berechtigung machen spätere
  UserOperations, die im selben Bundle bereits validiert wurden, nicht ungültig*.
  Siehe „Bekannte Probleme“ unten.
- **Nethermind**, August 2026: keine Befunde.

SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), das das Modul beim Bereitstellen einer Wallet
aktiviert, war von den Prüfungen durch Certora und Nethermind abgedeckt.

In der Geschichte des Moduls gibt es ein offengelegtes Problem: v0.1.0 signierte
`initCode` und `paymasterAndData` nicht, ein Gas-Griefing-Vektor, der
[in v0.2.0 behoben wurde](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
laut Safe wurde v0.1.0 außerhalb von Testnets nicht verwendet. Vela nutzt v0.3.0 mit
EntryPoint v0.7 und Safe 1.4.1, also die Konfiguration, die das Release des Moduls
beschreibt.

### Passkey-Modul von Safe v0.2.1 – die Signer

Dein erster Schlüssel wird vom **SafeWebAuthnSharedSigner** unter
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2` geprüft. „Shared“ bedeutet, dass die
Bereitstellung des Vertrags gemeinsam genutzt wird, so wie der Safe-Singleton; dein
Schlüssel wird nicht geteilt. Jedes Safe speichert seinen eigenen öffentlichen
P-256-Schlüssel in seinem eigenen Storage.

Jeder weitere Schlüssel hat einen eigenen Signer-Vertrag, den die
**SafeWebAuthnSignerFactory** unter `0x1d31F259eE307358a26dFb23EB365939E8641195` als
Proxy auf den **SafeWebAuthnSigner-Singleton** unter
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` erstellt.

Die Prüfungen, die diese Verträge in v0.2.1 abdecken
([Berichte](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- Ein [Audit-Wettbewerb von Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (Juni–Juli 2024): keine hohen oder mittleren Befunde; drei niedrige, alle behoben.
- **Certoras** Prüfung des Release-Commits: keine neuen Befunde. (Das frühere
  v0.2.0-Audit vermerkt, dass der Shared Signer noch nicht auditiert war – er kam erst
  nach diesem Audit hinzu.)
- **Nethermind**, August 2026: keine Befunde.

Seit dem Release wurde keine Schwachstelle auf Vertragsebene offengelegt, und die
Passkey-Verträge fallen unter das Bounty der Safe Foundation.

Passkey-Signaturen werden vom **EIP-7951/RIP-7212**-Precompile der Chain geprüft, ohne
Ersatz-Verifizierer. Bevor die App ein Netzwerk aktiviert, prüft sie das Precompile mit
einer echten Signatur. Zwei Einschränkungen: Die ursprüngliche RIP-7212-Spezifikation
hat Schwächen in Randfällen, die [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)
behebt (sie betreffen nur Eingaben, die ohnehin scheitern sollten, keine korrekt
gebildeten WebAuthn-Signaturen), und ein Test kann nicht jede Art erkennen, wie die
Implementierung einer Chain abweichen könnte.

### EntryPoint v0.7 – führt deine Operation aus

Bereitgestellt unter `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, dem
[kanonischen v0.7.0-Release](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Von OpenZeppelin auditiert](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
im Auftrag der Ethereum Foundation (Januar 2024): keine kritischen oder hohen Befunde,
fünf mittlere, alle 24 Befunde behoben; der Commit des Fix-Reviews entspricht dem
Release. Er fällt unter das
[ERC-4337-Bug-Bounty](https://docs.erc4337.io/community/bug-bounty) der Ethereum
Foundation (bis zu 250.000 US-Dollar).

## Bekannte Probleme, die wir beobachten

### Berechtigungsänderungen innerhalb eines Bundles (Safe4337Module, Certora M-01)

Der EntryPoint validiert jede Operation in einem Bundle, bevor er eine davon
ausführt. Entfernt also eine Operation einen Eigentümer, besteht eine Operation, die
dieser Eigentümer signiert hat und die später im selben Bundle steht, trotzdem die
Validierung und wird ausgeführt. Safe hat das zur Kenntnis genommen und v0.3.0 nicht
geändert.

Velas Apps bauen nie Eigentümerwechsel, Vela selbst löst das also nie aus. Relevant
bleibt es trotzdem: Eine dApp kann deine Wallet bitten, ihre eigenen Eigentümer zu
ändern (siehe „Lücken“ unten), und wer einen kompromittierten Schlüssel über andere
Safe-Werkzeuge entfernt, kann sich nicht darauf verlassen, dass dieser Schlüssel im
selben Bundle schon ausgesperrt ist.

### Abfangen einer signierten Operation (EntryPoint vor v0.9)

Im Februar 2026 legten Forschende einen Griefing- und Zensurvektor
[offen](https://erc4337.substack.com/p/improving-useroperation-execution), der jeden
EntryPoint vor v0.9 betrifft, auch v0.7. Wer eine signierte Operation in die Hände
bekommt, bevor sie in einen Block aufgenommen wird, kann sie innerhalb eines Aufrufs
ausführen, den er kontrolliert, und die innere Ausführung zum Revert zwingen: Die
Operation schlägt fehl und muss neu signiert werden. (Mit Velas In-Band-Gebühr wird die
Gebührenüberweisung mit zurückgesetzt, sodass das Relay und nicht du das Gas trägt.)
Betroffen sind Operationen, die Verträge mit Reentrancy-Schutz aufrufen oder sich durch
vorübergehenden Zustand zum Revert bringen lassen; einfache Überweisungen sind nicht
betroffen. Wiederholt gegen Auszahlungsabläufe eingesetzt, könnte das Geld eine Zeit
lang unzugänglich halten. Eine Signatur fälschen oder Geld umleiten kann es nicht.

Velas Relay reicht Operationen direkt ein statt über einen gemeinsamen Mempool, aber
eine ausstehende `handleOps`-Transaktion ist im öffentlichen Mempool trotzdem sichtbar –
das verkleinert die Angriffsfläche, beseitigt sie aber nicht. Die Behebung gibt es nur
in EntryPoint v0.9 (November 2025); v0.7 lässt sich nicht patchen. Eine Migration hängt
davon ab, dass das 4337-Modul von Safe v0.9 unterstützt, und diese Seite wird sagen,
wann es so weit ist.

### Lücken in Velas eigenen Schutzmechanismen

Keine Vertragsbefunde, sondern Stellen, an denen die Wallet dich weniger schützt, als
du vielleicht annimmst. Jede ist erfasst und soll behoben werden:

- **Aufrufe deiner Wallet an sich selbst werden nicht blockiert.** Eine dApp kann
  `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler` oder `setGuard` auf
  deinem eigenen Safe anfordern; jeder davon übergibt, einmal signiert, das Konto.
  Vela dekodiert diese Aufrufe, hält sie aber nicht auf. Lehne jede Anfrage ab, deren
  Ziel deine eigene Adresse ist.
- **Die Freigabesperre stoppt nur „unbegrenzte“ Beträge** (2^200 oder mehr; 2^152 bei
  Permit2). Eine große, aber begrenzte Freigabe, ein signiertes Permit oder ein
  NFT-`setApprovalForAll` bekommen einen Vorsichtshinweis, keine Sperre.
- **Abgerufene Deskriptoren sind nicht authentifiziert.** Ein Deskriptor vom
  Chain-Daten-Server wird als „verifiziert“ angezeigt, wenn er zum Vertrag passt; er
  ist nur so vertrauenswürdig wie dieser Server.
- **Die unabhängige Signaturseite ist noch an keine App angebunden.**
- **Die Netzwerkprüfung sucht nicht nach der Passkey-Signer-Factory von Safe**, die
  die Schlüssel zwei bis sieben brauchen; in einem Netzwerk, das ohne sie hinzugefügt
  wurde, kann nur der erste Schlüssel signieren.
- **Die Website lädt ein Analyse-Skript eines Drittanbieters** auf derselben Domain
  wie die Passkeys. Die Website verbietet ihren Seiten die Nutzung von Passkeys (über
  einen Permissions-Policy-Header) und hält das Skript von der Seite fern, auf der ein
  Schlüssel liegt.

## Was nicht auditiert ist

- **Velas eigene Verträge.** Das
  [Public-Key-Register](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  unter `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; dieselbe Adresse auf
  Ethereum und Base), die ursprüngliche Register-Bereitstellung unter
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (ihre Adresse ist Teil der
  Signatur-Domain jeder Registrierung) und der frühere Index, den sie abgelöst haben
  (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, nur noch lesbare Historie). Sie sind
  nicht auditiert. Sie halten kein Geld, haben keinen Eigentümer und lassen sich nicht
  upgraden; sie sind eine Auffindungsschicht, keine Autorisierungsschicht.
  Ausgabebefugnis kommt nur von den Schlüsseln, die in deinem Safe hinterlegt sind.
  Das schlimmste realistische Versagen ist, dass eine Wallet auf einem neuen Gerät
  schwerer zu finden ist, nicht, dass Geld bewegt wird.
- **Multicall3.** Seine README
  [sagt](https://github.com/mds1/multicall3) „This contract is unaudited.“ Vela nutzt es
  nur für gebündelte Lesezugriffe – Guthaben, Token-Details, Preisangebote –, nie mit
  Freigaben oder Geld.
- **Die deterministischen Deployer** (Arachnids CREATE2-Proxy und die Singleton-Factory
  von Safe) – Ökosystem-Standard und zustandslos, ohne formale Audits. Fehlen sie,
  lehnt Velas Netzwerkprüfung das Netzwerk ab; sie prüft, dass an der Adresse Code
  existiert, nicht, dass er Byte für Byte übereinstimmt.
- **Tempo.** Eines der 24 eingebauten Netzwerke, ohne nativen Coin; Vela zahlt das Gas
  dort im Stablecoin pathUSD. Stand September 2026 sagt Tempos
  [Sicherheitsrichtlinie](https://github.com/tempoxyz/.github/blob/main/SECURITY.md),
  dass das Protokoll noch auditiert wird und kein aktives Bug-Bounty hat. Guthaben auf
  Tempo und dort gezahltes Gas tragen dieses Risiko auf Chain-Ebene; betrachte Tempo
  als das neueste und am wenigsten erprobte Netzwerk der Liste.
- **Vela selbst.** Die Apps, die Backend-Dienste und die oben genannten Verträge hatten
  kein Audit durch Dritte, und derzeit ist auch keines angesetzt. Das ist der größte
  Vorbehalt auf dieser Seite. Die Details stehen in
  [Vela is in alpha](/blog/vela-is-in-alpha). Fang mit kleinen Beträgen an und lies den
  Code.

## Selbst prüfen

Jede Adresse unten ist eine kanonische öffentliche Bereitstellung. Prüfe sie gegen
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
und das [EntryPoint-Release](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Vertrag                                   | Adresse                                      |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2-Singleton v1.4.1                   | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner-Singleton v0.2.1       | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Public-Key-Register (Vela, nicht auditiert) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Wird beim Hinzufügen eines Netzwerks geprüft; dein Safe nutzt stattdessen das
4337-Modul als Fallback-Handler.
