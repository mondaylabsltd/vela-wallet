---
title: Audits und bekannte Probleme
description: Jeder On-Chain-Vertrag, von dem Vela abhängt, wer ihn auditiert hat, ob die auditierte Version dem entspricht, was tatsächlich läuft, und was gar nicht auditiert ist.
---

„Auditiert“ ist eine Aussage über eine bestimmte Version bestimmten Codes, deshalb
winkt diese Seite nicht mit dem Wort — sie nennt die genauen Berichte, die genauen
Deployment-Adressen und die Unterschiede zwischen auditierter und ausgerollter
Version. Sie listet auch, was _nicht_ auditiert ist, denn diese Liste trägt genauso
viel Gewicht wie die erste.

Zuletzt geprüft: August 2026. Findest du hier einen Fehler, sag uns Bescheid, wir
korrigieren ihn.

## Der Pfad des Geldes

Vier Vertragsschichten können dein Geld berühren. Alle vier sind
Drittanbieter-Verträge mit veröffentlichten Audits, und in jedem Fall ist die
Adresse das offizielle kanonische Deployment.

### Safe v1.4.1 — der Account selbst

Deine Wallet ist ein [Safe](https://github.com/safe-global/safe-smart-account)-Proxy:
SafeL2-Singleton, Proxy-Factory, Compatibility-Fallback-Handler und MultiSend fürs
Bündeln.

[Ackee Blockchain hat Safe v1.4.0 auditiert](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(Abschlussbericht März 2023): 11 Feststellungen, keine kritisch oder hoch. Das von
uns ausgerollte v1.4.1 unterscheidet sich vom auditierten v1.4.0 durch eine
einzeilige ERC-4337-Kompatibilitätskorrektur
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). Die Logik
von MultiSend ist seit dem
[von G0 Group auditierten v1.3.0](https://github.com/safe-global/safe-smart-account/tree/main/docs)
unverändert. Alle Adressen stimmen mit den kanonischen Deployments in
[safe-deployments](https://github.com/safe-global/safe-deployments) überein, und
die Verträge fallen unter das
[Bug-Bounty der Safe Foundation](https://docs.safefoundation.org/security/bug-bounty)
(bis zu 1.000.000 $ für kritische Funde).

Was ein Audit nicht abdeckt: den Bybit-Vorfall von 2025. Dieser Angriff
kompromittierte die Build-Pipeline von Safes offiziellem Web-Frontend, nicht die
Verträge — die
[offizielle forensische Schlussfolgerung](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
fand keine Schwachstelle in den Safe-Smart-Contracts. Wir lesen das als Lektion
über die Web- und Betriebsebene, also genau die Ebene, an der du auch uns messen
solltest.

### Safe4337Module v0.3.0 — der ERC-4337-Adapter

Ausgerollt unter `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, der kanonischen
v0.3.0-Adresse (Sourcify-Exaktabgleich — der On-Chain-Bytecode ist der auditierte
Code).
[Von Ackee Blockchain auditiert](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(Abschlussbericht März 2024), ohne offene Feststellungen oberhalb der
Informationsebene. Die Kombination v0.3.0 + EntryPoint v0.7 + Safe ≥ 1.4.1, die wir
verwenden, ist genau die Konfiguration, die Audit und Release Notes beschreiben.

Die Geschichte des Moduls enthält ein offengelegtes Problem: v0.1.0 (2023)
signierte `initCode` und `paymasterAndData` nicht, ein Gas-Griefing-Vektor. Das
wurde [in v0.2.0 behoben](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module),
und v0.1.0 hat die Testnetze nie verlassen. Wir nutzen v0.3.0, das die Korrektur
erbt.

### SafeWebAuthnSharedSigner v0.2.1 — der Passkey-Signer

Ausgerollt unter `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, der kanonischen
v0.2.1-Adresse (über die Safe-Singleton-Factory auf jeder Chain dieselbe).

Was „shared“ heißt — und was nicht: Geteilt ist das _Vertrags-Deployment_, so wie
das Safe-Singleton geteilt ist. Dein Schlüssel ist es nicht. Jeder Safe ruft
`configure()` per Delegatecall auf und legt seinen eigenen öffentlichen
P-256-Schlüssel in seinem eigenen Speicher ab. Eine Signer-Instanz steht für genau
einen Passkey pro Safe, und niemandes anderer Safe kann deinen benutzen.

Die Version zählt hier. Das Audit von v0.2.0
[stellte ausdrücklich fest](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md),
dass der Shared Signer nicht im Umfang war — den Vertrag gab es noch nicht. Die
Audits, die abdecken, was wir ausrollen, sind die von v0.2.1: ein
[Hats-Finance-Audit-Wettbewerb](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(Juni–Juli 2024: null hoch, null mittel, drei niedrige Feststellungen — alle
behoben) plus eine
[Certora-Prüfung des Release-Commits](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
ohne neue Feststellungen. Seit dem Release wurde keine Schwachstelle auf
Vertragsebene offengelegt; die Passkey-Verträge fallen unter das Bounty der Safe
Foundation.

Safes eigene Dokumentation empfiehlt, Passkey-Eigentum mit einem
Wiederherstellungspfad zu kombinieren, statt ein einzelnes Credential als einzigen
Schlüssel zum Account zu behandeln. Wie Vela das handhabt, steht in
[Wiederherstellung und Anmeldung](/de/docs/recovery).

Die On-Chain-P-256-Prüfung nutzt direkt die RIP-7212-Precompile, ohne
Solidity-Fallback-Verifier. Bevor ein Netzwerk aktiviert wird, testet die App die
Precompile mit einer echten Signatur und lehnt das Netzwerk ab, wenn die Prüfung
scheitert. Zwei ehrliche Einschränkungen: Die ursprüngliche RIP-7212-Spezifikation
hat Randfall-Mängel, für deren Korrektur
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) geschrieben wurde (sie
betreffen wohlgeformte WebAuthn-Signaturen nicht), und ein Test kann nicht jede
Art erwischen, wie die Implementierung einer Chain in ungewöhnlichen
Ausführungskontexten abweichen könnte.

### EntryPoint v0.7 — der ERC-4337-Einstiegspunkt

Ausgerollt unter `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, dem
[kanonischen v0.7.0-Deployment](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Von OpenZeppelin auditiert](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(im Auftrag der Ethereum Foundation, Januar 2024): null kritisch, null hoch, fünf
mittlere Feststellungen, alle behoben — und der auditierte Commit ist das
ausgerollte Release. EntryPoint v0.7.0 fällt unter das
[ERC-4337-Bug-Bounty](https://docs.erc4337.io/community/bug-bounty) der Ethereum
Foundation (bis zu 250.000 $).

## Bekannte Probleme, die wir beobachten

### Der EntryPoint-Griefing-Vektor

Im Februar 2026 legten Sicherheitsforscher von Trust Security einen Griefing- und
Zensurvektor
[offen](https://erc4337.substack.com/p/improving-useroperation-execution),
der jeden EntryPoint vor v0.9 betrifft, auch das von uns genutzte v0.7. Wer eine
signierte UserOperation abfängt, bevor sie eingeschlossen wird, kann sie in einem
selbst kontrollierten Call-Frame ausführen und die innere Ausführung zum Scheitern
zwingen — die Operation schlägt fehl, das Gas wird trotzdem berechnet. Die
Ethereum Foundation zahlte den Forschern 50.000 $ Bounty; sie stufte das Problem
als Zensur-/Griefing-Vektor ein, nicht als Vektor für Diebstahl, und es wurde nie
ausgenutzt.

Was er kann: eine Gebühr verbrennen und eine Transaktion verzögern. Was er nicht
kann: Geld stehlen oder eine Signatur fälschen. Velas Exposition ist schmal, weil
UserOperations direkt an ein Relay gehen statt durch einen öffentlichen Mempool —
es gibt also kaum Gelegenheit, eine abzufangen — und der schlimmste Fall ist durch
die Gebühr begrenzt, der du ohnehin zugestimmt hast. Die Korrektur existiert nur
in EntryPoint v0.9 (November 2025); v0.7 selbst lässt sich nicht patchen. Wir
rechnen mit einer Migration, sobald der umgebende Stack — insbesondere Safes
4337-Modulreihe — v0.9 unterstützt, und vermerken das dann hier.

## Was nicht auditiert ist

- **Velas eigene Verträge.** Zwei kleine Verträge, die wir selbst geschrieben und
  auf Gnosis ausgerollt haben: der
  [Passkey-Public-Key-Index](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (ein Append-only-Register, das deinen Geräten hilft, deinen öffentlichen
  Schlüssel zu finden) und sein Batch-Helfer. Sie sind nicht auditiert. Konstruktiv
  halten sie kein Geld, haben keinen Eigentümer und können nicht upgegradet werden
  — sie sind eine Auffindungsschicht, keine Autorisierungsschicht. Die Ausgabemacht
  kommt immer vom Passkey, der in deinem Safe konfiguriert ist. Der realistisch
  schlimmste Fehlerfall ist Griefing (jemand besetzt einen Index-Eintrag), was die
  Wiederherstellung unbequemer machen, aber kein Geld bewegen kann. Ein
  Gas-Abrechnungs-Splitter aus einem früheren Gebührendesign ist nicht mehr Teil
  des Transaktionsflusses.
- **Multicall3.** Die eigene README
  [sagt es deutlich](https://github.com/mds1/multicall3): „This contract is
  unaudited.“ Wir nutzen es genau so, wie seine Autoren es als sicher beschreiben —
  gebündelte Lesezugriffe für Guthaben, Token-Metadaten und Preise. Vela erteilt
  ihm nie Genehmigungen, und es hält nie Geld. Der schlimmste Fall eines Bugs ist
  ein falscher Lesewert.
- **Der CREATE2-Deployer.** Der
  [Arachnid Deterministic Deployment Proxy](https://github.com/Arachnid/deterministic-deployment-proxy)
  ist der zustandslose Standard-Deployer des Ökosystems; ein formales Audit hat er
  nicht. Unsere Netzwerkprüfungen schlagen sicherheitshalber fehl, wenn er auf
  einer Chain fehlt oder verändert ist.
- **Tempo und pathUSD.** Tempo, eines unserer zwölf eingebauten Netzwerke, hat
  keine native Coin; Gas wird dort im pathUSD-Stablecoin abgerechnet. Stand August
  2026 haben weder Tempos Kernprotokoll noch pathUSD ein veröffentlichtes
  Sicherheitsaudit oder ein Bug-Bounty, und eine unabhängige
  [DefiLlama-Sicherheitenbewertung](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (April 2026) stufte pathUSD als hochriskant ein. Das ist ein Risiko auf
  Chain-Ebene, das keine Wallet abfedern kann: Geld, das du auf Tempo hältst, und
  die dortige Gas-Abrechnung erben es. Behandle Tempo als die neueste und am
  wenigsten erprobte Chain der Liste und dimensioniere deine Guthaben entsprechend.
  Wir aktualisieren diesen Abschnitt, sobald Audits erscheinen.
- **Vela selbst.** Unsere App und die Backend-Dienste hatten kein Audit durch
  Dritte. Das ist die größte Einschränkung auf dieser Seite, wir sagen es im
  Seitenkopf, und die ehrlichen Details stehen in
  [Vela ist in der Alpha](/blog/vela-is-in-alpha). Fang mit kleinen Beträgen an.
  Lies den Code.

## Prüf es selbst

Jede Adresse oben ist ein öffentliches, kanonisches Deployment, das du gegen die
offiziellen Register abgleichen kannst —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
und die
[EntryPoint-Release-Notes](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Vertrag | Adresse |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Passkey-Public-Key-Index (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
