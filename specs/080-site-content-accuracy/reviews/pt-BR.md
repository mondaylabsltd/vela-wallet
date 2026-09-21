# pt-BR — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), pt-BR localizer · Method: single-string locale review, whole pages

Scope: every key the brief lists in `pt-BR.json`, plus the defects found in keys
it did not list; all 17 docs rewritten from the current English and Chinese
(`self-hosting.md` created). The old pt-BR docs were translated from an English
that predates spec 080, so most of what follows is accuracy — a Brazilian reader
was being told things the product no longer does, or never did.

## Terminology and register

| Concept | pt-BR | Note |
| --- | --- | --- |
| key (any signing credential) | chave | umbrella term, as the wallet UI uses it |
| passkey | passkey (feminine: *a passkey*) | 059 choice, kept |
| security key / hardware security key | chave de segurança / chave de segurança física | *física* is what the wallet's own pt-BR onboarding says (`custodyBody`) and what Google Brasil calls a YubiKey-class key; replaces *chave de hardware* / *chave de segurança de hardware* |
| signer (a Safe owner) | signatário | **changed** from *assinante*, which a Brazilian reader parses first as "subscriber"; owners = *proprietários* |
| relay | relay (masculine: *o relay*) | kept in English, as the product (`vela-relay`, UI "Vela Relay") and Brazilian crypto writing do; *relayer* only for the relay's own sender addresses in the self-hosting guide |
| registry | registro / contrato de registro | the on-chain contract; the UI's *registro* already used on the landing seal |
| public-key index | índice de chaves públicas | the service; the Settings field is quoted by its UI label, *índice de passkey* |
| self-hosting | auto-hospedagem / hospedar (ou rodar) por conta própria | matches the wallet UI link "Guia de auto-hospedagem →"; hyphen required before *h* |
| signing page | página de assinatura | |
| clear signing / blind signing | assinatura legível / assinatura às cegas | 059 choice, kept; matches the wallet corpus (*Transação às cegas*) |
| seed phrase / recovery phrase | frase de recuperação | **docs changed** from *frase-semente* so the docs and the catalog use one term (the catalog, footer and MetaMask pt-BR all say *frase de recuperação*) |
| iCloud Keychain / Google Password Manager | Chaves do iCloud / Gerenciador de senhas do Google | Apple's and Google's own Brazilian names |
| chain data | dados de chain | as in the Settings label *Índice de dados da chain*; *rede* for "network" everywhere else |
| precompile / deploy / run | pré-compilado / implantar / rodar | |

Register: *você*, as recorded in 059, with você-form imperatives; the brand takes
the article (*a Vela*), the account is *o Safe* and the company *a Safe*. Brazilian
usage throughout — *celular*, *tela*, *arquivo*, *aparelho* — and Brazilian
formatting: US$ 0,01, 140.000, 1,1 milhão, curly “ ” quotes. UI labels are quoted
as the pt-BR builds show them: the wallet corpus (*Configurações*, *Endpoints de
serviço*, *Restaurar padrões*, *Criar carteira*, *Receber*/*Enviar*,
*Lento*/*Padrão*/*Rápido*, *Verificado*), Chrome (*Modo do desenvolvedor*,
*Carregar sem compactação*) and Windows SmartScreen (*O Windows protegeu o
computador*, *Mais informações*, *Executar assim mesmo*). Contract names,
EIP/ERC numbers, commands, file paths and addresses stay as in English; comments
inside code blocks are translated, as `zh` does.

## Findings

Catalog (`pt-BR.json`):

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `home.meta.description`, `ogDescription`, `organization` | "Assine com uma passkey — sem frase de recuperação, **sem chave de hardware**, sem ficar preso a ninguém" | mistranslation (stale) | High | denies the security-key support C-keys-2 describes; "self-hostable" overclaims (C-rp-1) | passkeys ou chaves de segurança; conta Safe sem modificações; serviços de código aberto |
| 2 | `home.hero.facts[3]` | "Se a Vela parar de funcionar, você continua com acesso…" / "Hospede você mesmo o app, o relay e cada serviço de backend" | mistranslation (stale) | High | promise the passkey domain limit contradicts (C-rp-1) | "Tudo o que a Vela roda… você pode rodar por conta própria" / what still depends on us |
| 3 | `home.hero.facts[2].link` | "…e o caminho que fechamos" | mistranslation | Medium | claims the attack path is closed; it is mitigated | "e o que a Vela faz a respeito" |
| 4 | `home.why.p1` | href `account.base.app`; "se o serviço sumir, a carteira some junto" | technical + mistranslation | High | href differed from English (test failure); the complaint is now the closed signing service | new href, "(hoje parte da Coinbase Wallet)", "serviço de assinatura cujo código não é público" |
| 5 | `home.tradeoffs.items[0]` | "a taxa inclui o custo on-chain e uma taxa pelo serviço de relay"; link to GitHub; "trocar de relayer nas configurações" | mistranslation (stale) | High | hides C-fee-1 (3 × reserved gas, often 10× the real cost) | full fee rule, `/docs/self-hosting#relay`, "apontar a carteira para outro relay" |
| 6 | `home.tradeoffs.items[1].body` | "Você pode definir várias chaves ao criar a carteira" (no full stop) | mistranslation (stale) | High | drops "up to seven" and "can't be changed afterwards" (C-keys-1) | "até sete chaves… não podem ser alteradas depois" |
| 7 | `home.tradeoffs.items[2]` | "Você está confiando em contratos Safe auditados — e uma auditoria não é uma garantia" | mistranslation (stale) | High | never says Vela's own code is unaudited, which the ledger requires of this card (C-audit-1) | "O código da própria Vela, não" + "nenhuma está agendada" |
| 8 | `home.compare.rows` | 12 rows; "Passkey" only; MetaMask "EOA", sponsored gas "Não oferecido"; "App, relay e serviços de backend, tudo pode rodar por sua conta"; "página ou extensão independente que você hospeda" | mistranslation (stale) | High | missing "Adding a key later" row; three cells false (MetaMask sponsors on some networks; self-hosting has gaps; signing page not connected, C-signpage-1) | 13 rows from the reference; *Suportado* → *Disponível* in cells |
| 9 | `home.pricing.cards` | "Apps de desktop e celular — Grátis pelo código"; "Nas lojas — Compra única" | mistranslation (stale) | Medium | desktop is a free download, not source-only; store apps are not out yet (C-plat-1) | three cards from the reference, "Compra única · em breve" |
| 10 | `home.networks.heading`, `.body` | "12 redes embutidas"; external biubiu.tools link | mistranslation (stale) + technical | High | 24 networks (C-net-1, test failure); link now `/chain-setup` | "24 redes integradas"; *pré-compilado* |
| 11 | `home.faq.items[0].a` | "Um aparelho que abre com Face ID ou digital, ou uma chave de segurança USB/NFC" | mistranslation (stale) | Medium | face/fingerprint as the only check (C-auth-1); no "two if only security keys", no "up to seven" | from the reference |
| 12 | `home.faq.items[1].a` | "coloca a carteira direto dentro da dApp… apps de iOS, Android e desktop" | mistranslation (stale) | Medium | omits that the web wallet doesn't connect and that desktop means macOS/Windows (C-dapp-1) | from the reference |
| 13 | `home.faq.items[2].a` and 8 docs | "Chaveiro do iCloud" | UI fit | Medium | not Apple's Brazilian name (that is *Chaves do iCloud*); a reader looking for it in Ajustes won't find it | "Chaves do iCloud" everywhere |
| 14 | `home.faq.items[4].a` | "Quem chegar às suas passkeys sincronizadas provavelmente chega também à sua carteira" | mistranslation (stale) | High | no remedy: a key can't be removed, so move the funds | from the reference |
| 15 | `home.faq.items[5].a` | "A Vela não pode mover nem congelar seus fundos. Só as suas chaves controlam a carteira." | mistranslation (stale) | High | drops "it writes the software that asks your keys to sign" and half of what is public / seen (IP, key labels, authenticator kind) | from the reference |
| 16 | `home.faq.items[6].a` | "assine pelo navegador: a extensão da Vela, ou a extensão de assinatura legível sem dependências" | mistranslation (stale) | High | offers the unpublished, unconnected signing page as a way in (C-signpage-1) | the Vela extension and self-built apps, with the keys each can use |
| 17 | `about.lede`, `team.bio`, `values[0..2]` | "a carteira, os contratos inteligentes…"; "Suas chaves, suas moedas — não é slogan"; "uma passkey: seu rosto ou sua digital" | mistranslation (stale) + tone | Medium | Vela writes no contract in the funds path (C-acct-1); face/fingerprint only (C-auth-1); slogan voice | from the reference |
| 18 | `roadmap.upcoming`, `.shipped` | "…uma auditoria de segurança independente da integração Safe + WebAuthn"; "caminho de assinatura para chains sem o precompilado P-256"; "suas contas já te acompanham pelo backup" | mistranslation (stale) | High | lists an audit as upcoming (forbidden, A02 FR-2), a path C-p256-1 rules out, and sync C-sync-1 denies; arrays were 5/7, not 5/10 (test failure) | both arrays replaced from the reference |
| 19 | `getStarted.meta.description`, `lede`, `platforms.web.blurb`, `platforms.desktop.stores`, `fundingNote` | "desktop, celular e uma extensão… construídos do mesmo código"; "Mac App Store"; "o mesmo app, sem custo" | mistranslation (stale) | High | phone apps not available; not on the Mac App Store; a self-built phone app can't use the phone's own passkey (C-plat-1, C-rp-1) | from the reference |
| 20 | `chrome.docs.titles.signers` and the doc | "Assinantes e chaves de segurança" | terminology | Medium | *assinante* reads as "subscriber" in Brazil | "Signatários e chaves de segurança" |
| 21 | `home.seal.label`, `.verify` | "carteiras criadas on-chain"; "ver o registro" | mistranslation | Low | the English now says *registered*; a link reads better as an imperative | "registradas"; "veja o registro" |
| 22 | `chrome.docs.titles["clear-signing-self-host"]` | "Hospede você a página de assinatura" | unnatural | Low | misplaced pronoun | "Hospedar a página de assinatura" |
| 23 | `chrome.docs.titles["create-wallet"]` | "Crie sua carteira" | consistency | Low | the only imperative among infinitive sidebar titles | "Criar sua carteira" |
| 24 | `home.why.heading` | "Por que construímos" | consistency | Low | same English as `nav.whyVela` ("Por que a criamos"), rendered differently and without an object | "Por que a criamos" |
| 25 | catalog + docs | *a dApp* (FAQ) vs *os dApps* (Get Vela) | consistency | Low | two genders for one noun | masculine *os dApps* throughout |

Docs (`content/docs/pt-BR/`):

| # | File | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 26 | `introduction.md`, `install.md` | "12 redes"; "não há nada para baixar nem loja de apps"; "você assina com uma passkey, usando o rosto ou a digital" | mistranslation (stale) | High | C-net-1, C-plat-1, C-auth-1 | rewritten from the reference |
| 27 | `networks-and-fees.md` | "conta de gas", "depósito de ativação não reembolsável", "Não há seletor de velocidade", "custo de rede mais a taxa de serviço" | mistranslation (stale) | High | mechanism gone; speed is selectable; fee rule is C-fee-1 (C-fee-2, C-fee-3) | rewritten; treasury top-up is optional and non-refundable |
| 28 | `whitepaper.md` | "Sem margem escondida"; "conta de relay dedicada… ativada por um depósito"; "o app e os quatro serviços de backend… licenciado MIT" | mistranslation (stale) | High | C-fee-1, C-fee-2, C-lic-1 (the index has no licence file) | rewritten |
| 29 | `faq.md` | "12 redes EVM"; "depósito não reembolsável"; "a carteira e seus quatro serviços… sob licença MIT" | mistranslation (stale) | High | C-net-1, C-fee-2, C-lic-1 | rewritten |
| 30 | `account-contract.md` | "qualquer interface compatível com Safe" consegue operar a conta | mistranslation (false) | High | tools can read and build, only getvela.app-capable software can sign (C-safeui-1) | rewritten |
| 31 | `recovery.md` | "Sua recuperação depende do chaveiro da plataforma" as the recovery model | mistranslation (stale) | High | sign-in with any one key, rebuilt from the registry (C-sync-1, C-keys-2) | rewritten |
| 32 | `clear-signing.md` | "Aprovações ilimitadas são bloqueadas… ela reescreve o pedido" | mistranslation (stale) | High | drops the caveat that signed permits and large finite approvals are only cautioned (C-approve-1) | "Aprovações on-chain “ilimitadas” não podem ser enviadas" + what it does not stop |
| 33 | `bybit-attack.md` | no self-call gap; signing page "será opcional" | mistranslation (stale) | High | omits the `enableModule`/`addOwnerWithThreshold` exposure and that no app sends requests to the page | rewritten |
| 34 | `security-audits.md` | "uma das nossas doze redes"; no EntryPoint < v0.9 interception issue, no "Gaps in Vela's own defences" | mistranslation (stale) | High | missing security disclosures the English now makes | rewritten |
| 35 | `install.md`, `create-wallet.md` | no `<span id="dapps">` / `<span id="what-is-public">` | technical | High | deep links from other pages landed on the page top (test failures) | anchors in place |
| 36 | `signers.md` | description: "não é uma limitação que esquecemos de remover"; no compromised-key section | mistranslation (stale) + tone | Medium | defensive tone; missing "move everything to a new wallet" | rewritten |
| 37 | all docs | *frase-semente* | terminology | Medium | catalog says *frase de recuperação*; one concept, two terms in one locale | *frase de recuperação* |
| 38 | 10 of 16 old docs (`why-vela.md`, `create-wallet.md`, `clear-signing.md`…) | «…» quotation marks | unnatural / cultural | Medium | guillemets are the European Portuguese convention; Brazilian text uses “ ” | “ ” throughout |

Also: `self-hosting.md` did not exist in pt-BR, so every `/pt-BR/docs/self-hosting#…`
link in the other pages had no target; it is now written, with all six anchors.

High/Medium fixed: 23 High, 10 Medium (findings 1–20, 26–38).

## Open items

- **Split / sweep labels.** The wallet's pt-BR corpus has no user-facing label
  for the two multi-recipient modes, so `send-and-receive.md` names them
  *Dividir* and *Consolidar*. If the apps later label them differently, align
  the doc to the app.
- **"Avançado".** The path *Configurações → Avançado → Endpoints de serviço*
  uses *Avançado* for the section, which is not in the pt-BR corpus as a string;
  the modal title *Endpoints de serviço* is. Conservative choice; confirm against
  a pt-BR build.
- `review.json` was not touched (outside this task's files); whether pt-BR moves
  from `drafted` to `reviewed` is the founder's call.

## Result

reviewed — no open High or Medium findings
