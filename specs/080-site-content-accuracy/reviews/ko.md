# ko — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), acting as native Korean localizer · Method: single-string locale review, whole pages

Scope: `messages/ko.json` (every key the brief lists, the chrome drafts, and three unlisted keys
with real defects) and all 17 docs in `content/docs/ko/`, `self-hosting.md` new. Every page was
rewritten from the current English and Chinese. The old Korean was translated from an older
English and had been drafted, never reviewed (059 `reviews/ko.md`); it was reused only for
phrasing whose meaning still matched (chiefly `why-vela`, `signers`, `bybit-attack`).

## Terminology and register

| Concept | ko | Note |
| --- | --- | --- |
| key (any signing credential) | 키 | counted with 개 (`최대 일곱 개`), as the app does |
| passkey | 패스키 | 059 choice, kept |
| security key | 보안 키 / 하드웨어 보안 키 / USB 보안 키 | the app's words (`onboarding.create.methodSecurityKeyTitle` = `USB 보안 키`) |
| relay | 릴레이; relay fee 릴레이 수수료 | the old files mixed 릴레이 and 릴레이어. ERC-4337's role name given once as 번들러. vela-relay's internal hot wallets are 릴레이어 (self-hosting only), as the relay's own docs call them |
| registry | 레지스트리 (공개 키 레지스트리) | the app says 레지스트리 |
| public-key index | 공개 키 인덱스 | the service; the Settings field is named by its app label 패스키 인덱스 |
| self-hosting | 셀프 호스팅 (guide: 셀프 호스팅 가이드); verb 직접 운영하다 / 직접 호스팅하다 | sidebar group 직접 운영하기 |
| signing page | 서명 페이지 | |
| clear signing / blind signing | 클리어 서명 / 블라인드 서명 | 블라인드 서명 is the app's term; 클리어 서명 is this site's established term (see Open items) |
| seed phrase / recovery phrase | 시드 구문 / 복구 구문 | the old catalog said 시드 구문 and the old docs 시드 문구; unified on 구문, which is also MetaMask Korean's word |
| self-custodial | 자기 수탁(형) | the old docs wrote 자기수탁, the catalog 자기 수탁; unified |
| transaction / operation | 거래 / 오퍼레이션 | 거래 is the app's word (`거래 내역`, `거래 속도`); UserOperation left in English |
| relying party / authenticator | 신뢰 당사자(relying party) / 인증자 | standard Korean WebAuthn terms; English given once in parentheses |
| descriptor / treasury | 디스크립터 / 트레저리 | both as the app writes them |
| custom network | 사용자 지정 네트워크 | the app's word (`settings.networks.custom`); the old text had 커스텀 |
| Bybit | 바이비트 | how Korean press writes it; the incident is 바이비트 해킹 사건 |

UI paths are the app's Korean labels, not translations of the English ones: 설정 → 고급 →
서비스 엔드포인트, 설정 → 네트워크, speeds 느림 / 표준 / 빠름, buttons 보내기 / 받기 / 지갑 만들기,
key choices 이 기기 / 휴대폰 또는 태블릿 / USB 보안 키, fields 체인 데이터 · 패스키 인덱스 ·
Vela 릴레이 · 법정화폐 환율, 기본값으로 재설정, tags 검증됨 / 미검증 / 무제한 / 철회. Chrome's own
Korean button 압축해제된 확장 프로그램을 로드합니다 and Windows SmartScreen's Windows의 PC 보호 /
추가 정보 / 실행 are used as those programs show them.

Register: 합니다체 throughout, as in 059; table cells and list fragments as noun phrases. One
deliberate departure from the old text: the rewritten strings drop `당신` wherever Korean leaves
the subject implicit, which is nearly everywhere — the old docs used it 97 times, which reads as
translated English. Where a subject is needed the text says 사용자 or 내 (`내 지갑 주소`). The four
unlisted, founder-approved catalog strings that use 당신 (`hero.subtitle`, `hero.facts[0].term`,
`tradeoffs.lede`, `tradeoffs.close`) were left alone; the register is unchanged, so they do not clash.

## Findings

Catalog (`messages/ko.json`):

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `home.meta.description`, `.organization` | `패스키로 서명합니다 — 시드 구문도, 하드웨어 키도, 잠금도 없습니다` | mistranslation | High | says there are **no hardware keys**; security keys are a first-class key type (C-keys-2). Also "for ETH and ERC-20", gone from the English | `패스키나 보안 키로 서명하며, 시드 구문은 없습니다. 계정은 수정하지 않은 Safe이고…` |
| 2 | `home.meta.ogDescription` | `패스키 서명, 시드 구문 없음, 잠금 없음. 원한다면 직접 빌드하세요.` | mistranslation | Medium | stale: no unmodified Safe, no security keys, no open services | rewritten from current English |
| 3 | `home.hero.facts[2].link` | `…우리가 막는 방법` | mistranslation | Medium | "how we stop it" — the linked page says the attack is mitigated, not eliminated | `…그리고 Vela의 대응` |
| 4 | `home.hero.facts[3].term` | `Vela가 서비스를 멈춰도 당신의 지갑에는 계속 접근할 수 있습니다.` | mistranslation | High | an unconditional promise the reference does not make; access without getvela.app depends on the extension or a self-built app (C-rp-1) | `Vela가 지갑을 위해 운영하는 것은 모두 오픈소스이고, 직접 운영할 수도 있습니다.` |
| 5 | `home.hero.facts[3].link` | `앱도 릴레이도 모든 백엔드 서비스도 직접 돌릴 수 있습니다` | mistranslation | Medium | drops "what still depends on us" | `직접 운영할 것, getvela.app 없이도 작동하는 것, 여전히 우리에게 의존하는 것` |
| 6 | `home.seal.label`, `.verify` | `온체인에 생성된 지갑` / `온체인에 있습니다` | technical | Medium | wallets are **registered**; the contract itself is counterfactual and not created at registration | `온체인에 등록된 지갑` / `온체인에 등록됩니다` |
| 7 | `home.why.p1` | href `https://account.base.app`; `브라우저에서 생성되어… 복구 키` | technical | High | href no longer matches the English (FR-031 test failed); the closed signing service point was missing | new href, `(지금은 Coinbase Wallet의 일부)`, `코드가 공개되지 않은 서명 서비스` |
| 8 | `home.tradeoffs.items[0]` | `수수료에는 온체인 비용과 릴레이 서비스 이용료가 포함되며…`; link to the vela-relay repo | mistranslation | High | understates the fee: no 3× reserve, no "often ten times or more", no $0.01 minimum (C-fee-1); link target changed in English | rewritten; link `/docs/self-hosting#relay` |
| 9 | `home.tradeoffs.items[1].body` | `지갑을 만들 때 키를 여러 개 등록할 수 있습니다` (no full stop) | mistranslation | High | missing "up to seven" and "can't be changed afterwards" (C-keys-1) | `최대 일곱 개까지 정할 수 있고, 나중에는 바꿀 수 없습니다.` |
| 10 | `home.tradeoffs.items[2]` | `당신이 믿는 것은 감사를 받은 Safe 컨트랙트이고 — 감사는 보증이 아닙니다` | mistranslation | High | never says Vela's own code is unaudited with none scheduled — the sentence C-audit-1 requires here | `컨트랙트는 감사를 받았습니다. Vela 자체 코드는 받지 않았습니다.` + `예정된 감사도 없습니다` |
| 11 | `home.compare.rows` | 12 rows | technical | High | English has 13 (`Adding a key later` added, `Open source` → `Source code`); `ko.home is not a fragment` failed | 13 rows; `나중에 키 추가`, `소스 코드` |
| 12 | `home.compare.rows[9].vela` | `독립적이고 직접 호스팅하는 페이지나 확장으로 확인` | mistranslation | High | presents the unpublished, unconnected signing page as a working check (C-signpage-1) | `독립 서명 페이지. 만들어졌지만 아직 앱과 연결되지 않음` |
| 13 | `home.compare.rows[10].vela` | `앱, 릴레이, 백엔드 서비스까지 직접 운영 가능` | mistranslation | High | omits the listed gaps and that passkeys stay tied to getvela.app (C-rp-1) | rewritten with both |
| 14 | `home.compare.rows[1].vela`, `.base` | `패스키` / `패스키` | mistranslation | Medium | Vela: security keys and "up to seven" missing; Base: recovery phrase missing | `패스키 또는 보안 키, 최대 일곱 개` / `패스키 또는 복구 구문` |
| 15 | `home.compare.rows` MetaMask cells | `EOA`; sponsored gas `없음`; batching `계정 기능에 따라 다름` | mistranslation | Medium | contradict the current English (7702 by default, sponsorship on some networks) | rewritten |
| 16 | `home.compare.rows[11]` | `오픈소스 범위: 앱, 릴레이, 백엔드 서비스` | mistranslation | Medium | licence nuance lost (index has no licence; C-lic-1); MetaMask's non-commercial licence missing | `인덱스를 뺀 나머지는 MIT이고, 인덱스는 아직 라이선스가 없음` |
| 17 | `home.pricing.cards[1]` | `데스크톱·모바일 앱` / `소스로 빌드하면 무료` | mistranslation | Medium | desktop is a free download, not source-only (C-plat-1) | card 0 `웹, 브라우저 확장 프로그램, 데스크톱`; card 1 `직접 빌드한 iPhone·Android 앱` |
| 18 | `home.pricing.cards[2]` | `앱스토어에서 받으면` / `한 번만 결제` | mistranslation | High | reads as available now; the phone apps are not in the stores yet | `1회 구매 · 출시 예정` |
| 19 | `home.networks.heading` | `12개 네트워크 기본 탑재…` | mistranslation | High | 24 (C-net-1); `networks.test.ts` failed | `24개 네트워크 기본 탑재. 직접 추가도 가능` |
| 20 | `home.networks.body` | link to `biubiu.tools/apps/vela-wallet-chain-setup` | technical | High | href differs from the English `/chain-setup` (FR-031 test failed) | `<a href="/chain-setup">체인 설정</a>` |
| 21 | `home.faq.items[0].a` | `Face ID나 지문으로 잠금을 해제하는 기기, 또는 USB/NFC 보안 키` | mistranslation | Medium | missing "two, if you use only security keys" | added |
| 22 | `home.faq.items[1].a` | `iOS·Android·데스크톱 앱과 Vela 브라우저 확장에서 모두 됩니다` | mistranslation | Medium | implies Linux desktop; omits that the web wallet does not connect (C-dapp-1) | `데스크톱(macOS, Windows)…웹 지갑은 dApp에 연결하지 않습니다` |
| 23 | `home.faq.items[4].a` | `동기화된 패스키에 닿은 사람은 당신의 지갑에도 닿을 수 있습니다.` | mistranslation | High | the safety instruction is missing: a key can't be removed, so move funds to a new wallet | rewritten |
| 24 | `home.faq.items[5].a` | `지갑을 움직이는 것은 당신의 키뿐입니다… 공개 키와 지갑 주소, 지갑 이름` | mistranslation | Medium | drops "Vela writes the software that asks your keys to sign"; public-data and IP lists incomplete | rewritten |
| 25 | `home.faq.items[6].a` | `…또는 의존성 없는 클리어 서명 확장` | mistranslation | High | names the unpublished signing page as a way in when getvela.app is down (C-signpage-1) | extension + self-built apps only |
| 26 | `about.lede` | `뒤에 얼굴 없는 회사는 없습니다` | mistranslation | Medium | contradicts the current English, which names MONDAY LABS LTD | rewritten |
| 27 | `about.values[0].body` | `당신의 키, 당신의 코인 — 구호가 아니라 구조입니다` | mistranslation | Medium | drops "what we do control is the software you sign with" | rewritten |
| 28 | `about.values[1].body` | `암호자산을 잃는 가장 큰 원인은…`; `당신의 얼굴이나 지문으로` | mistranslation | Medium | unsupported claim; face/fingerprint as the only check (C-auth-1) | `얼굴, 지문, PIN으로 확인한 뒤…`; security keys named |
| 29 | `roadmap.upcoming` | `…Vela의 Safe + WebAuthn 통합에 대한 독립 보안 감사`; `P-256 프리컴파일이 없는 체인을 위한 서명 경로` | mistranslation | High | promises an audit (forbidden, A02 FR-2) and a non-P-256 path (C-p256-1); `계정과 네트워크가 플랫폼 백업을 통해 따라옵니다` contradicts C-sync-1 | new 5-item array |
| 30 | `roadmap.shipped` | 7 stale items | technical | High | English has 10; `ko.roadmap is not a fragment` failed | new 10-item array |
| 31 | `getStarted.lede` | `…데스크톱으로, 휴대폰으로, 브라우저 도구 모음으로 가져가세요.` | mistranslation | Medium | phone offered as available now | `곧 휴대폰에서도 쓸 수 있습니다.` |
| 32 | `getStarted.platforms.web.blurb` | `열고 패스키로 인증하면…` | mistranslation | Medium | missing "to connect to dApps, use the extension"; any key, not only a passkey | rewritten |
| 33 | `getStarted.platforms.desktop.stores` | `Mac App Store · Microsoft Store` | mistranslation | Medium | the English lists Microsoft Store only | `Microsoft Store` |
| 34 | `getStarted.fundingNote` | `직접 빌드해도 됩니다 — 같은 앱이고, 비용은 없습니다.` | mistranslation | High | a self-built phone app is **not** the same: it can't use the phone's own passkey (C-rp-1); a reader could build it and lose access to their wallet on that phone | caveat added |
| 35 | `about.team.bio` | `지갑, 컨트랙트, 그리고 이 사이트까지` | mistranslation | Low | Vela does not write the contracts that hold funds | `앱, 서비스, 이 사이트까지` |
| 36 | `chrome.docs.titles.bybit-attack` | `바이비트 공격` | unnatural | Low | Korean press says 해킹 (사건) | `바이비트 해킹 사건` (unlisted key) |
| 37 | `home.hero.facts[0].link` | `당신의 계정은 Safe의 컨트랙트이지…` | unnatural | Low | repeats `당신의 계정은` from the term directly above it | `계정 컨트랙트는 우리가 아니라 Safe가 만든 것입니다` (unlisted key) |

Docs (`content/docs/ko/`):

| # | File | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 38 | all docs vs catalog | `시드 문구` / `시드 구문`, `자기수탁` / `자기 수탁`, `릴레이` / `릴레이어`, `커스텀` / `사용자 지정` | terminology | Medium | one concept, two words, between the docs and the catalog | unified (table above) |
| 39 | `introduction` | `12개 네트워크, 하나의 주소.` | mistranslation | High | 24 (C-net-1) | `24개 네트워크, 하나의 주소.` |
| 40 | `introduction` | `Vela 설치 — 브라우저에서 돌아가므로 내려받을 것이 없습니다.` | mistranslation | High | contradicts C-plat-1 (extension and desktop are downloads, phones are paid) | page restructured as the new question table |
| 41 | `introduction` | `서명은 얼굴이나 지문으로 쓰는 패스키로 합니다`; `승인 전에 모든 거래를 읽을 수 있습니다` | mistranslation | Medium | one kind of key and one check only (C-keys-2, C-auth-1); "every transaction readable" overclaims — undecodable calls are labelled | rewritten |
| 42 | `install` | `내려받을 것도 없고 앱스토어를 거칠 필요도 없습니다` | mistranslation | High | C-plat-1 | rewritten with the four-row table |
| 43 | `install` | `안드로이드 9 이상`; `계정은… 온체인에 있으므로, 패스키와 지갑이 그대로 따라옵니다` | mistranslation | Medium | Android 10+; account lists are not synced (C-sync-1) | rewritten |
| 44 | `install` | no `id="dapps"` | technical | High | anchor linked from elsewhere; test failed | `<span id="dapps"></span>` in "dApp에서 Vela 쓰기" |
| 45 | `create-wallet` | `Vela는 패스키의 공개 키에서 주소를 유도하므로…` | mistranslation | High | the address comes from **all** founding keys (C-addr-1) | `앱이 전체 키 목록으로 지갑 주소를 계산하고…` |
| 46 | `create-wallet` | one passkey, no mention of up to seven keys or that they are fixed | mistranslation | High | C-keys-1 | steps 3–5 and the warning callout |
| 47 | `create-wallet` | no `id="what-is-public"` | technical | High | anchor linked from four pages; test failed | added, with the full public-record list (C-reg-1) |
| 48 | `why-vela` | `직접 호스팅 가능하게 만든 것은 당신의 지갑이 우리 회사가 살아 있는지에 기대지 않도록…` | mistranslation | High | the domain limit is missing (C-rp-1) | `한계는 하나 있습니다. 패스키가 속한 도메인입니다…` |
| 49 | `why-vela` | callout link `Safe 스마트 계정` → `/ko/docs/security-audits`; no "the app decides what your key is asked to sign" | technical | Medium | wrong link target; the one caveat the callout exists to state was missing | `/ko/docs/account-contract`; sentence added |
| 50 | `why-vela` | `맨 처음 키를 하드웨어 보안 키로 삼을 수 있습니다` | mistranslation | Medium | security-key-only wallets need **two** keys | `하드웨어 보안 키만 써도 됩니다. 이때는 두 개가 필요합니다.` |
| 51 | `send-and-receive` | `무제한 토큰 승인은… 제출을 거부합니다` | mistranslation | High | no permit caveat (C-approve-1) | section moved to `clear-signing`, which carries the caveat |
| 52 | `send-and-receive` | `패스키(Face ID / Touch ID / 지문)를 요청합니다` | mistranslation | Medium | C-auth-1 | `Face ID, 지문, PIN, 또는 보안 키를 터치하고 PIN을 입력` |
| 53 | `networks-and-fees` | `12개 EVM 네트워크` | mistranslation | High | 24 (C-net-1) | 24-row table |
| 54 | `networks-and-fees` | `가스 계정 활성화… 환불되지 않는 보증금` | mistranslation | High | the gas account no longer exists (C-fee-2) | `지갑별 가스 계정이나 활성화 보증금은 없습니다.` |
| 55 | `networks-and-fees` | `속도 선택지는 없습니다.` | mistranslation | High | there is a speed choice, default fast (C-fee-3) | `느림 / 표준 / 빠름` |
| 56 | `networks-and-fees` | `릴레이의 제시가 곧 가격이고…` | mistranslation | High | the fee formula is 3 × reserved gas × the higher price, often 10×+ (C-fee-1) | full formula |
| 57 | `passkeys` | `얼굴이나 지문으로 인증했을 때만 쓰인다`; private key only in the OS keychain | mistranslation | Medium | C-auth-1, C-keys-2 (security keys, other phones) | three-row key table |
| 58 | `passkeys` | no "a passkey signs whatever you approve" warning | mistranslation | Medium | the page's main caveat was absent | warning callout |
| 59 | `signers` | `생성 시 YubiKey를 등록하고 그것으로 서명하면 됩니다` | mistranslation | Medium | a wallet with one unsynced key can't be created; two security keys | rewritten |
| 60 | `recovery` | recovery described as the platform keychain only | mistranslation | Medium | any one key, including a phone or security key, signs in | rewritten |
| 61 | `recovery` | `Vela는 패스키 서명 두 번으로… 지갑 주소를 다시 유도할 수 있습니다` | mistranslation | Medium | true only for single-key wallets | `키가 하나뿐인 지갑은…` callout |
| 62 | `clear-signing` | `무제한 승인은 차단됩니다` | mistranslation | High | no permit / large-finite / setApprovalForAll caveat (C-approve-1) | `'무제한' 온체인 승인은 제출할 수 없습니다` + what it does not stop |
| 63 | `clear-signing` | `Vela가 18이라고 넘겨짚는 일은 없습니다` | mistranslation | High | inverted: Vela does display as 18 decimals and marks it unverified | `18자리라고 가정하고… 미검증으로 표시` |
| 64 | `clear-signing` | `검증됨` unqualified | mistranslation | Medium | not cryptographic (C-clear-1) | explained |
| 65 | `bybit-attack` | `공격이 기댄 업그레이드라는 수단을 없애고` | mistranslation | High | inverted: the owner-signed `delegatecall` primitive still exists in every Safe | `…모든 Safe에 여전히 존재합니다` |
| 66 | `bybit-attack` | `서명마다 생체 인증을 새로 합니다`; status line without "no app sends requests yet"; `누구도 신뢰하지 않아도 되는 답은 그것뿐` | mistranslation | Medium | C-auth-1; C-signpage-1; self-built code is still trusted | rewritten |
| 67 | `account-contract` | `Safe 호환 인터페이스라면 무엇이든 다룰 수 있습니다` | mistranslation | High | any Safe tool can read, only getvela.app-capable software can sign (C-safeui-1) | rewritten |
| 68 | `account-contract` | `Vela 컨트랙트라는 것은 없습니다`; gas `1.5~3배`; audit scope `앱 코드` | mistranslation | Medium | the registry contracts exist (outside the funds path); measured 140–170k vs 21k; C-audit-1 names apps, services and registry | rewritten |
| 69 | `security-audits` | `내장 네트워크 열두 개` | mistranslation | High | 24 | `24개 내장 네트워크` |
| 70 | `security-audits` | Vela itself: no "none scheduled", registry not named; interception `가스는 청구됩니다`, `공개 멤풀을 거치지 않고… 기회가 거의 없고` | mistranslation | Medium | C-audit-1; the relay, not the user, absorbs the gas; pending `handleOps` is still visible | rewritten |
| 71 | `clear-signing-self-host` | no status line | mistranslation | High | built but not published, no app sends requests (C-signpage-1) | `**상태:**` paragraph |
| 72 | `clear-signing-self-host` | `자체 네트워크 요청도 없습니다` | mistranslation | Medium | it fetches decorative token logos | `유일한 요청은 장식용 토큰 로고` |
| 73 | `whitepaper` | `12개 EVM 네트워크`; `가스 계정… 환불되지 않는 보증금` | mistranslation | High | C-net-1, C-fee-2 | rewritten |
| 74 | `whitepaper` | trust summary `감사된 스마트 컨트랙트, OS의 패스키 보관소, 그리고 — 가용성만을…` | mistranslation | Medium | omits the app's code and the domain | five-item trust list |
| 75 | `faq` | `12개 EVM 네트워크`; `가스 릴레이 계정을 활성화하기 위한… 보증금` | mistranslation | High | C-net-1, C-fee-2 | rewritten |
| 76 | `faq` | `지갑과 네 가지 백엔드 서비스… MIT` | mistranslation | High | the index has no licence file (C-lic-1) | rewritten |
| 77 | `faq` | `Vela가 저장하는 것은 패스키의 공개 키와… 이름뿐` | mistranslation | Medium | services also see IP, operations, lookups | rewritten |

Totals: **39 High** (#1, 4, 7–13, 18–20, 23, 25, 29, 30, 34, 39, 40, 42, 44–48, 51, 53–56, 62, 63,
65, 67, 69, 71, 73, 75, 76) and **35 Medium** (17 in the catalog, 18 in the docs), plus 3 Low —
counting a row that bundles several defects once, at its highest severity. All fixed.

Checked mechanically as well as read: every link target in each doc matches the English with the
`/ko` prefix added (blog, privacy, terms, registry left unprefixed); callouts, anchors, fences,
headings, inline code, bold markers, table rows and pipe counts are the same per page; contract
addresses unchanged; no hard line break lands inside a Korean word (a newline renders as a space).

## Open items

- **Split / sweep labels.** The app's Korean corpus has no labels for the two batch-send modes, so
  the docs describe them (`나눠 보내기`, `모아 보내기`). If the app later names them, the docs
  should use those names.
- **"best effort".** No app label exists; written `추정 해석(best effort)`. Conservative; align with
  the app if it adds one.
- **클리어 서명.** Kept as this site's established term (sidebar, docs). Korean industry usage
  also has `클리어 사이닝`, and the app leaves "Clear Signing" untranslated in its developer
  screen. A file-wide terminology decision, not changed here.
- **Windows SmartScreen wording** (`Windows의 PC 보호` / `추가 정보` / `실행`) is kept from the
  existing catalog and was not re-checked on a Korean Windows install.
- `review.json` still says `drafted` for ko; it is outside the files this pass may edit.

## Result

reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the en + zh revision of the same day (fee wording, configurable relay chain
directory, hero subtitle, facts #3 and #4) into ko. Each changed string checked on the
five single-string axes; no High or Medium left open.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | 서명은 당신의 기기에서 이뤄집니다. 패스키의 개인 키는 절대 Vela에 전달되지 않습니다. | — | 개인 키 as `passkeys.md` says; 당신 kept, as in the other founder-approved hero strings |
| `home.hero.facts[2]` | 보이는 그대로 서명합니다. Vela는 승인하기 전에 실제로 서명할 거래를 디코딩합니다. / …그리고 Vela가 서명할 내용을 보여 주는 방법 | — | no settled Korean term for WYSIWYS; 보이는 그대로 서명 is the plain idiom and answers the old term 화면에 보이는 것이 반드시… |
| `home.hero.facts[3]` | term = 059 string; link Vela가 사라져도 지갑을 계속 쓰는 방법 | — | 059 Vela가 서비스를 멈춰도 당신의 지갑에는 계속 접근할 수 있습니다. **kept**: same meaning as en and zh, natural. Finding #4 against it predates the en reinstatement; the overclaiming 059 link is not restored |
| `home.tradeoffs.items[0].body` | paragraph 2 rewritten (one fee to the relay, Vela's unless changed; formula + `#fee` link 계산 방식; relay pays the gas and keeps the rest) | — | both hrefs identical to en; paragraph 3 unchanged |
| `home.faq.items[6].a` | code-change sentence replaced by the four services you can run | — | second paragraph untouched |
| `roadmap.upcoming[1].body` | chain-data clause removed | — | |
| docs `networks-and-fees` | `<span id="fee">`; "10배 이상" paragraph replaced; **수수료는 누가 받나.** paragraph | — | |
| docs `faq` | cost bullet (relay choice, `#fee` link); shutdown answer without the code-change parenthesis | — | |
| docs `whitepaper` | intro sentence dropped; Fees bullet + new "who gets the fee" bullet; "Vela가 사라진다면" sentence dropped | — | re-wrapped at spaces only |
| docs `self-hosting` | intro limit dropped; `VELA_RELAY_CHAIN_DIRECTORY_URL` comment lines in both code blocks; 알아 둘 것 bullet; chain-data paragraph; relay line removed from the final list | — | 체인 디렉터리 / Vela의 사본 as the file already said |

Nothing fixed beyond the brief.

## Update 2026-09-22 (audience)

Carried the en + zh audience revision (commit 66a3c789: copy for the reader who self-hosts
and builds from source; p256-index now MIT) into ko. Each changed string checked on the five
single-string axes; no High or Medium left open. `messages.test.ts -t "ko"` passes.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.meta.description` / `.ogDescription` / `.organization` | build-and-host framing; organization 자기 수탁형 → 셀프 호스팅 가능한 | — | |
| `home.hero.ctaSelfHost` (new) | 또는 전체 스택을 직접 운영하기 | — | inserted after `ctaCode`; matches the 「… 직접 운영하기」 headings |
| `home.why.more` | 긴 버전 — 다른 지갑에서 받아들일 수 없었던 것, 그리고 우리가 택한 트레이드오프 | — | |
| `home.compare.rows` | reordered to the new en order; Source code cell → 「…공개</a>, 모두 MIT」 | — | anchor unchanged |
| `home.faq.items` | new order; three new answers and the merged lose-a-key/account answer | — | 합니다체 answers; the new first answer avoids a 해요체 "빼고요" and says 「…만 빼면 모두 가능합니다」; questions keep the file's 「…면요?」 form |
| `getStarted.lede` | + 모든 앱은 소스에서 빌드할 수도 있습니다. | — | |
| docs `introduction` | opening now 셀프 호스팅할 수 있는 … 오픈소스 지갑; 「우리 없이도 돌아갑니다」 bullet first and rewritten; table reordered with the new relay/fee row | — | re-wrapped at spaces only |
| docs `faq`, `self-hosting`, `whitepaper` | all MIT including the index; (Rust, MIT); licence-file sentence deleted | — | |

Nothing fixed beyond the brief.

## Update 2026-09-22 (P-256 naming)

Added the two-names sentence to `networks-and-fees` (EIP-7951이며 / RIP-7212입니다, 롤업) after the signature-check sentence.
Read every `EIP-7951 / RIP-7212` line in the docs and `home.networks.body`: the particle still attaches to RIP-7212 (…RIP-7212를 지원해야), so all read correctly; nothing else changed.
