# es-MX — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), es-MX localizer · Method: single-string locale review, whole pages

Scope: every key the brief lists in `es-MX.json`, plus the defects found in keys it
did not list; all 17 docs rewritten from the current English and Chinese
(`self-hosting.md` created). The old es-MX docs were translated from an English
that predates spec 080 (12 networks, a gas account with an activation deposit, one
passkey synced by iCloud or Google), so most findings below are accuracy: a Mexican
reader was being told things the product no longer does, or never did.

## Terminology and register

| Concept | es-MX | Note |
| --- | --- | --- |
| key (any signing credential) | llave | umbrella term, as the file and the wallet UI already use it |
| passkey | passkey (feminine: *la passkey*) | 059 choice, kept |
| security key / hardware security key | llave de seguridad / llave de seguridad física | *física* is what the wallet's own es-MX onboarding text says and what Google's Latin American Spanish calls a YubiKey-class key; replaces *llave de hardware* |
| signer (a Safe owner) | firmante; owners = dueños | |
| relay | relay (masculine: *el relay*) | kept in English, as the product (`vela-relay`, UI field "Vela Relay") does; **unified** — the old files mixed *relay* and *relayer*; *relayer* now only for the relay's own sender addresses in the self-hosting guide |
| registry | registro / contrato de registro | *contrato de registro* at first mention in a section, because *registro* alone also means "record" |
| public-key index | índice de llaves públicas | the service; the Settings field is quoted by its UI label, *Índice de passkey* |
| self-hosting | autoalojamiento / alojar tú mismo; run a service = operar | matches the wallet UI link "Guía de autoalojamiento →"; *operar* for running a service (relay, index), *ejecutar* for running a program |
| signing page | página de firma | |
| clear signing / blind signing | firma legible / firma a ciegas | 059 choice, kept |
| seed phrase / recovery phrase | frase semilla / frase de recuperación | *frase semilla* (the file's and the app's term) for seed phrases; *frase de recuperación* only for what Coinbase calls its own recovery phrase |
| iCloud Keychain / Google Password Manager | Llavero de iCloud / Administrador de contraseñas de Google | Apple's and Google's Latin American names, as the es-MX wallet corpus writes them; **changed** from the Spain form *Gestor de contraseñas de Google* |
| wallet | wallet (feminine: *la wallet*) | the site's established term, kept; stray *cartera* removed (see F-31). The in-app button is quoted as the app labels it: **Crear billetera** |
| fee / base fee | comisión / tarifa base, tarifa de prioridad | *tarifa* only for the EIP-1559 terms, so it never competes with Vela's *comisión* |
| deploy / precompile / on-chain | desplegar / precompilado / on-chain | as the file already used them |

Register: informal *tú*, as recorded in 059, with tú-form imperatives and no
*vosotros*. Mexican usage throughout: *celular*, *computadora*, *laptop*, *roomies*,
*toparse con*, *ingresar*, in a neutral product-documentation register. Mexican
formatting: US$0.01, 1,500 millones, 140,000, 1.8 veces, dates as *21 de febrero de
2025*, «» quotes as the file already used. UI labels are quoted as the es-MX builds
show them: the wallet corpus (*Ajustes → Redes*, *Ajustes → Avanzado → Endpoints de
servicio*, *Índice de datos de cadena*, *Índice de passkey*, *Vela Relay*, *Tipos de
cambio fiat*, *Restablecer a los valores predeterminados*, *Crear billetera*,
*Recibir*/*Enviar*, *Lento*/*Estándar*/*Rápido*, *Verificado*, *Muy pronto*),
Chrome (*Modo de desarrollador*, *Cargar descomprimida*) and Windows SmartScreen
(*Windows protegió su PC*, *Más información*, *Ejecutar de todas formas*). The
privacy link reads *Política de privacidad*, not the Mexican legal term *Aviso de
privacidad*, which would imply a LFPDPPP notice that the English page is not.
Contract names, EIP/ERC numbers, commands, file paths and addresses stay as in
English; comments inside code blocks and the prose of the whitepaper's text diagram
are translated, as `zh` does.

## Findings

| # | File / key | Before (old translation) | Type | Severity | Why | After |
|---|---|---|---|---|---|---|
| F-1 | `home.meta.description` | «…autoalojable… Firma con una passkey: sin frase semilla, **sin llave de hardware**…» | mistranslation | High | contradicts C-keys-2 (security keys are a first-class key kind) and states full self-hostability without the domain limit | «…que puedes hacer funcionar sin nosotros. Firma con passkeys o llaves de seguridad, sin frase semilla. Tu cuenta es un Safe sin modificar…» |
| F-2 | `home.meta.ogDescription` | «…autoalojable… Compílala tú mismo si quieres.» | mistranslation | Medium | old claim set; drops the unmodified Safe and security keys | «Wallet de Ethereum de código abierto sobre un Safe sin modificar. Passkeys o llaves de seguridad…» |
| F-3 | `home.meta.organization` | «…Firma con una passkey: sin frase semilla, **sin llave de hardware**…» | mistranslation | High | same as F-1, in structured data search engines quote | «…de autocustodia, construida sobre un Safe sin modificar, que firma con passkeys o llaves de seguridad…» |
| F-4 | `home.hero.facts[2].link` | «…y el camino que cerramos» | mistranslation | Medium | overclaims; the reference now says "what Vela does about it" | «…y qué hace Vela al respecto» |
| F-5 | `home.hero.facts[3].term` / `.link` | «Aunque Vela deje de operar, sigues teniendo acceso a tu wallet.» / «Aloja tú mismo la app, el relayer y cada servicio de backend» | mistranslation | High | unconditional promise contradicted by C-rp-1 (passkeys stay bound to getvela.app) | «Todo lo que Vela opera para tu wallet es de código abierto, y puedes operarlo tú mismo.» / «Qué operar tú mismo, qué sigue funcionando sin getvela.app y qué depende todavía de nosotros» |
| F-6 | `home.seal.label` / `.verify` | «wallets creadas on-chain» / «Cada wallet está on-chain» | mistranslation | Medium | the counter counts registry records, not deployed wallets (a wallet deploys on first send) | «wallets registradas on-chain» / «Cada wallet está registrada on-chain — consulta el registro» |
| F-7 | `home.why.p1` | href `account.base.app`; «una llave de recuperación generada en un navegador…» | technical + mistranslation | High | href no longer matched English (test failed); missing the closed signing service, "recovery phrase on a website" | new href; «una frase de recuperación creada en un sitio web… y un servicio de firma cuyo código no es público…» |
| F-8 | `home.tradeoffs.items[0]` | «La comisión incluye el costo on-chain y una comisión por el servicio de relay… Puedes cambiar de relayer en la configuración» | mistranslation | High | contradicts C-fee-1 (3× reserved gas, often ≥10× the real cost) and C-relay-1; href changed to `/docs/self-hosting#relay` | rewritten to the formula, the $0.01 minimum, "no puede cambiar después", «apuntar la wallet a otro relay u operar el tuyo» |
| F-9 | `home.tradeoffs.items[1].body` | «Puedes definir varias llaves al crear tu wallet» (no period) | mistranslation | High | drops "up to seven" and "can't be changed afterwards" (C-keys-1) | «Eliges hasta siete llaves al crear la wallet, y después no se pueden cambiar.» |
| F-10 | `home.tradeoffs.items[2]` | «Estás confiando en contratos Safe auditados — y una auditoría no es una garantía» | mistranslation | High | the reference's point — Vela's own code is not audited and none is scheduled (C-audit-1) — was absent | «Los contratos están auditados. El código propio de Vela, no.» + «…no hay ninguna programada» |
| F-11 | `home.compare.rows` | 12 rows | technical | High | English has 13 (new "Adding a key later"); the namespace was a fragment and failed the whole-or-absent test | 13 rows, «Agregar una llave después» in place |
| F-12 | `compare` "Revisión adicional" / "Autoalojamiento completo" | «Con una página o extensión independiente que alojas tú» / «App, relay y servicios de backend, todo lo puedes correr tú» | mistranslation | High | presents the unpublished signing page as available (C-signpage-1) and full self-hosting without the domain gap (C-rp-1) | «…ya construida pero todavía no conectada a las apps» / «…con las carencias que enumera la guía de autoalojamiento; las passkeys siguen ligadas a getvela.app» |
| F-13 | `compare` signing key, gas, sponsored gas, networks, source code | «Passkey»; «Patrocinado donde hay soporte»; «Alcance del código abierto: App, relay y servicios de backend» | mistranslation | Medium | facts updated in the reference (security keys, up to seven; licence status C-lic-1; fixed network list; MetaMask 7702) | rows retranslated from the 080 English |
| F-14 | `home.pricing.cards` | «Apps de escritorio y móvil — Gratis desde el código»; «Desde las tiendas — Pago único» | mistranslation | Medium | desktop is a free download; store apps are not available yet (C-plat-1) | «Web, extensión de navegador y escritorio — Gratis»; «iPhone y Android, desde las tiendas — Pago único · muy pronto» |
| F-15 | `home.networks.heading` / `.body` | «12 redes integradas…»; link to the biubiu tool | mistranslation + technical | High | C-net-1 (24); count test failed; href now `/chain-setup` | «24 redes integradas. Agrega la tuya»; «la página de configuración de cadenas te dice cuáles…» |
| F-16 | `home.faq.items[0].a` | «Un dispositivo que se desbloquee con Face ID o huella…» | mistranslation | Medium | C-auth-1/C-keys-2; missing "two, if only security keys" and "up to seven" | «Un celular o una computadora compatibles con passkeys, o llaves de seguridad físicas (dos, si solo vas a usar llaves de seguridad)…» |
| F-17 | `home.faq.items[1].a` | «Funciona en las apps de iOS, Android y escritorio…» | mistranslation | Medium | C-dapp-1: implies Linux desktop, omits that the web wallet doesn't connect | «…escritorio (macOS, Windows), iPhone y Android. La wallet web no se conecta a dApps.» |
| F-18 | `home.faq.items[4].a` | «…usa mejor una llave de seguridad USB/NFC.» | mistranslation | Medium | missing "a key can't be removed — move your funds" | rewritten; «…una llave no se puede quitar. Pasa tus fondos a una wallet nueva…» |
| F-19 | `home.faq.items[5].a` | «Solo tus llaves controlan la wallet… Tu llave pública, la dirección… son públicos» | mistranslation | Medium | incomplete public data (C-reg-1), missing what services see, missing "it writes the software that asks your keys to sign" | retranslated in full |
| F-20 | `home.faq.items[6].a` | «…firma desde el navegador: la extensión de Vela, **o la extensión de firma legible sin dependencias**» | mistranslation | High | offers the unpublished signing page as a way in (C-signpage-1, C-rp-1) | «…la extensión de Vela para el navegador sigue firmando… y lo mismo hacen las apps que compiles tú…» |
| F-21 | `about.lede` / `about.team.bio` | «la wallet, **los contratos inteligentes** y este mismo sitio» | mistranslation | Medium | contradicts C-acct-1 (no contract in the funds path is Vela's) | «las apps, los servicios de backend y este sitio…» |
| F-22 | `about.values[0..2]` | «La reemplazamos por una passkey: tu cara o tu huella.» | mistranslation | Medium | C-auth-1; values[0] lacked "we control the software you sign with" | retranslated |
| F-23 | `roadmap.upcoming` | «…y una auditoría de seguridad independiente de la integración Safe + WebAuthn»; «una vía de firma para cadenas sin el precompilado P-256»; «tus cuentas ya te siguen a través del respaldo de tu plataforma» | mistranslation | High | promised an audit (forbidden: none is scheduled), a P-256 fallback (C-p256-1) and account sync (C-sync-1) | the five new items |
| F-24 | `roadmap.shipped` | 7 old items | technical | High | English has 10; fragment test failed | the ten new items |
| F-25 | `getStarted.meta.description` / `.lede` | «…versiones de escritorio, móvil y una extensión construidas desde el mismo código» | mistranslation | Medium | phone apps are not available yet | «…apps de escritorio que puedes descargar ya, y apps para celular en camino» |
| F-26 | `getStarted.platforms.web.blurb` | «Ábrela, autentícate con tu passkey…» | mistranslation | Medium | missing "use the extension for dApps" (C-dapp-1) and "any of your keys" | retranslated |
| F-27 | `getStarted.platforms.desktop.stores` | «Mac App Store · Microsoft Store» | mistranslation | Medium | there is no Mac App Store listing | «Microsoft Store» |
| F-28 | `getStarted.fundingNote` | «Todo es de código abierto… la misma app, sin costo.» | mistranslation | Medium | a self-built phone app can't use the phone's own passkey (C-rp-1) | «…Eso sí: una app de celular compilada por ti firma con otro celular o con una llave de seguridad…» |
| F-29 | whole locale | *relay* and *relayer* used for the same thing | terminology | Medium | one concept, two words across catalog and docs | *relay* everywhere; *relayer* only for sender addresses |
| F-30 | `home.faq.items[2].a`, docs | «Gestor de contraseñas de Google» | UI fit | Low | Spain naming; Google and the es-MX app say *Administrador de contraseñas de Google* | fixed (not in the brief's list; recorded here) |
| F-31 | `compare` MetaMask cells, `pricing.cards[0]` | «carteras de hardware», «El cliente de la cartera», «Cartera web» | terminology | Low | the locale says *wallet* everywhere else | *wallet* |
| F-32 | `chrome.docs.groups.selfHost` (draft) | «Alójalo tú» | mistranslation | Low | the group is "Run it yourself" (services, not just hosting) | «Ejecútalo tú mismo»; `titles["clear-signing-self-host"]` → «Aloja tú mismo la página de firma» |
| D-1 | docs/introduction | «12 redes…»; «firmas con una passkey, con tu cara o tu huella»; «corre en tu navegador, no hay nada que descargar» | mistranslation | High | C-net-1, C-keys-1/2, C-plat-1 | rewritten from the 080 page |
| D-2 | docs/install | «no hay nada que descargar ni tienda de apps»; no `dapps` anchor | mistranslation + technical | High | C-plat-1; linked anchor missing (test failed) | rewritten; `<span id="dapps">` in the dApps section |
| D-3 | docs/create-wallet | single-passkey flow; no `what-is-public` anchor | mistranslation + technical | High | C-keys-1, C-reg-1; anchor test failed | rewritten; anchor in place |
| D-4 | docs/why-vela | «Luego las passkeys cambiaron lo que una wallet puede sentirse»; «chocábamos con bordes»; «una llave de recuperación sentada en una pestaña» | unnatural | Medium | ungrammatical calque and literal English images | «…cambiaron cómo podía sentirse una wallet»; «nos topábamos con límites»; «abierta en una pestaña del navegador» |
| D-5 | docs/send-and-receive | no split/sweep, no address names, «El relayer…», «Antes de darle enviar» | mistranslation | Medium | sections added in 080 were missing; terminology; typo | rewritten |
| D-6 | docs/networks-and-fees | «12 redes EVM»; «cuenta de gas… depósito de activación»; «No hay selector de velocidad»; fee = network cost + service fee | mistranslation | High | C-net-1, C-fee-1, C-fee-2, C-fee-3 all contradicted | rewritten |
| D-7 | docs/passkeys | «usada solo con tu cara o tu huella»; key held «normalmente» by iCloud/Google | mistranslation | High | C-auth-1, C-keys-2 (phones, security keys, other managers) | rewritten with the three kinds of key |
| D-8 | docs/signers | no app-support table; no "if a key may be compromised" section | mistranslation | Medium | the 080 page adds both; the second is safety guidance | rewritten |
| D-9 | docs/recovery | «Inicia sesión en la misma cuenta de iCloud o Google…» as the way back | mistranslation | High | C-keys-2, C-sync-1: any one key, registry fallback, Ethereum copy | rewritten |
| D-10 | docs/clear-signing | «Las aprobaciones ilimitadas se bloquean» (no caveats); "verificada" unqualified | mistranslation | High | C-approve-1 (finite approvals, permits, setApprovalForAll not capped), C-clear-1 (not cryptographic) | rewritten |
| D-11 | docs/account-contract | «cualquier interfaz compatible con Safe puede manejarla» | mistranslation | High | C-safeui-1: reading yes, signing needs a getvela.app signature | rewritten |
| D-12 | docs/security-audits | «Última revisión: agosto de 2026»; «doce redes integradas» | mistranslation | High | C-net-1; stale review date; 080 adds known issues and gaps | rewritten |
| D-13 | docs/clear-signing-self-host | no status line | mistranslation | High | C-signpage-1: built, not published, no app sends requests | rewritten with the status paragraph |
| D-14 | docs/whitepaper | «cuenta de relay dedicada… depósito no reembolsable»; keys only via iCloud/Google; 12 networks | mistranslation | High | C-fee-2, C-keys-2, C-net-1 | rewritten |
| D-15 | docs/faq | «depósito no reembolsable para activar su cuenta de relay de gas»; 12 networks | mistranslation | High | C-fee-2, C-net-1 | rewritten |

Counts: **25 High** and **19 Medium** found in the old text, all fixed; 3 notable Low
fixed. `self-hosting.md` is new (no old text to review): all six anchors, the
Callouts and every code block kept, only code comments translated.

## Open items

- **In-app wording vs site wording.** The es-MX wallet UI says *billetera*; the site
  says *wallet* (059 register, kept). Button labels are quoted as the app shows them
  (**Crear billetera**), so a reader may see both words. Aligning the two is a
  product decision, not a translation fix; left as is.
- **Split / sweep labels.** No es-MX UI string for the two batch modes was found in
  the corpus, so the docs name them descriptively (**Repartir**, **Juntar**). If the
  app ships labels, the doc should quote them.
- **Readiness.** This pass was a reading by the session that wrote the text, not by a
  native Mexican-Spanish speaker; the 059 note that a native read is still owed
  applies.

## Result
reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the en + zh revision of the same day (fee wording, configurable relay chain
directory, hero subtitle, facts #3 and #4) into es-MX. Each changed string checked
on the five single-string axes; no High or Medium left open.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | «La firma se hace en tu dispositivo. La llave privada de tu passkey nunca llega a Vela.» | — | *llave privada*, as `passkeys.md` and `create-wallet.md` already say (the locale's *llave* umbrella), not the more generic *clave privada* |
| `home.hero.facts[2]` | «Lo que ves es lo que firmas: Vela decodifica la transacción exacta antes de que la apruebes.» / «…y cómo Vela te muestra lo que firmas» | — | *Lo que ves es lo que firmas* is the Spanish rendering of WYSIWYS and mirrors the existing callout «Lo que ves es lo que pagas» |
| `home.hero.facts[3]` | term = 059 string, kept; link «Cómo seguir usando tu wallet si Vela desaparece» | — | 059 «Aunque Vela deje de operar, sigues teniendo acceso a tu wallet.» **kept**: it says what en ("doesn't depend on Vela staying online") and zh (即使 Vela 停止服务…) say, in natural Mexican Spanish. The link reuses the whitepaper heading «Si Vela desaparece» |
| `home.tradeoffs.items[0].body` | paragraph 2 rewritten (one fee, to the relay; Vela's unless changed; formula + `#fee` link; relay keeps the rest); paragraph 3 tail aligned | — | both hrefs identical to en |
| `home.faq.items[6].a` | code-change clause replaced by the four services you can run | — | second paragraph untouched |
| `roadmap.upcoming[1].body` | chain-data clause removed | — | |
| docs `networks-and-fees` | `<span id="fee">`; "ten times" paragraph replaced; **Quién la recibe.** paragraph | — | |
| docs `faq` | cost bullet (relay choice, `#fee` link); shutdown answer without the code-change parenthesis | — | parentheses rather than a dash: es-MX files avoid em dashes |
| docs `whitepaper` | intro clause dropped; Fees bullet + new "who gets the fee" bullet; "Si Vela desaparece" clause dropped | — | |
| docs `self-hosting` | intro limit dropped; `VELA_RELAY_CHAIN_DIRECTORY_URL` comment lines in both code blocks; "Toma en cuenta" bullet; chain-data paragraph; relay line removed from the final list | — | *directorio de cadenas*, as the removed list line named it |

Nothing fixed beyond the brief.
