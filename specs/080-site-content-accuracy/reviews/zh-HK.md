# zh-HK — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), native zh-HK localizer · Method: single-string locale review, whole pages

Scope: `app-web/getvela.app/src/lib/i18n/messages/zh-HK.json` (every key the brief lists, plus every
other key, because the register changed — see below) and all 17 docs under
`app-web/getvela.app/src/content/docs/zh-HK/` (`self-hosting.md` is new). Every doc was rewritten from the
current English and Simplified Chinese. The old zh-HK text was used only where its terminology still fit;
nothing was character-converted from `zh` or `zh-TW`.

## Terminology and register

**Register: written Standard Chinese for Hong Kong readers (書面語), not spoken Cantonese.** This is a
deliberate departure from spec 059, which wrote this locale in spoken Cantonese (「我哋」「唔使」「嘅」「喺」).
Reasons:

1. The 080 brief sets written Standard Chinese as the default for zh-HK. The parent task added that Hong
   Kong product copy is normally written that way.
2. The file was already mixed: the `chrome.docs.ui` drafts were written Chinese and everything else was
   Cantonese. It had to be made consistent one way or the other.
3. The Vela app's own zh-HK corpus has been moved to written Hong Kong Chinese (spoken Cantonese there was
   classed as the wrong register, and `batchRateHint`/`batchParsedCount` were rewritten out of it). The site
   and the app should read the same.
4. The locale switcher names this language 繁體中文（香港）, not 廣東話. Security, audit and whitepaper pages
   read more naturally, and are easier to scan, in written Chinese, as Apple HK and Google HK support pages are.

Everything is written with Hong Kong vocabulary and forms: 軟件、硬件、網絡、設定、帳戶、資料、檔案、資料夾、
登入、電郵、伺服器、程式碼、原始碼、支援、預設、透過、內置、模組、韌體、試算表、聯絡人、私隱、網誌、轉賬、
美仙、橫額、手提電腦; 甚麼、裏、着 (HK standard forms). The user is addressed as plain 你, as in 059.
Punctuation is full-width with 「」 for quotes and —— for dashes. Product, contract and network names,
EIP/ERC numbers, commands, addresses, file paths and code stay in English; code comments are translated.

| Concept | zh-HK | Note |
| --- | --- | --- |
| key (any signing credential) | 鑰匙 | the umbrella term. The old text mixed 鎖匙 (spoken), 金鑰 (Taiwan) and 鑰匙 |
| passkey | 通行密鑰 | 059 term, kept |
| security key | 安全密鑰 / 硬件安全密鑰 | was 安全金鑰. 金鑰 is the Taiwanese word (the same issue as 059 zh-HK-1). Apple HK uses 安全密鑰, it pairs with 通行密鑰, and the app's create-wallet button says 「USB 安全密鑰」 |
| private / public key | 私鑰 / 公鑰 | |
| signer (key) / signer contract | 簽署鑰匙 / 簽署器（合約） | shared signer = 共享簽署器 |
| sign (verb) / signature (noun) | 簽署 / 簽名 | HK written usage (簽署交易); the noun stays 簽名 |
| clear signing / blind signing | 清晰簽署 / 盲簽 | 059 sidebar term, and the app's term |
| signing page | 簽名頁 | 059 sidebar term, kept |
| relay | 中繼 | the ERC-4337 role is 打包器（bundler）. A build-tool bundler is 打包工具 so the two can't be confused (059 zh-HK-2) |
| registry | 註冊表; verb 登記 | was 登記表 in `seal.verify` (登記表 reads as a registration form) |
| public-key index | 公鑰索引 | the app's field label 「通行密鑰索引」 is quoted where the field is meant |
| self-hosting | 自行架設; guide 自行架設指南 | the draft chrome term. Self-custody is 自我託管, kept separate (zh uses 自托管 for both) |
| relying party | 依賴方（relying party） | |
| descriptor | 描述檔 | |
| authenticator | 驗證器 | |
| audit | 審計 | 059 term |
| create (a wallet, a key) | 建立 | the app says 建立錢包 |

UI labels are quoted as the Vela app's zh-HK corpus shows them (`assets/i18n/zh-HK.json`):
收款、發送、活動、設定 → 網絡、設定 → 進階 → 服務端點、重設為預設值、「區塊鏈數據索引」「通行密鑰索引」
「VELA RELAY」「法定貨幣匯率」、較慢／標準／超快. Chrome's are 開發人員模式 and 載入未封裝項目, and Windows
SmartScreen's are 「Windows 已保護您的電腦」、其他資訊、仍要執行.

## Findings

The old zh-HK text was translated from an older English. Most High findings are facts the reference has
since corrected, still asserted in this locale. Old text is quoted as it was, in spoken Cantonese.

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | whole locale (`zh-HK.json` + 16 docs) | spoken Cantonese throughout, with written-Chinese drafts in `chrome.docs.ui` | cultural risk / terminology | Medium | two registers in one locale; see *Terminology and register* | written Standard Chinese with HK vocabulary everywhere |
| 2 | `notice.offer.message` / `.accept` | 呢版都有廣東話版。/ 用廣東話睇 | UI fit | Medium | calls the locale "Cantonese" while the switcher calls it 繁體中文（香港） | 此頁亦有繁體中文（香港）版本。/ 閱讀繁體中文（香港）版 |
| 3 | "security key" in all files; `chrome.docs.titles.signers`, `chrome.docs.groups.keys` | 安全金鑰; 簽署金鑰同安全金鑰; 金鑰同復原 | terminology | Medium | Taiwanese 金鑰 in a Hong Kong locale; conflicts with 通行密鑰 and the app | 安全密鑰; 簽署鑰匙與安全密鑰; 鑰匙與復原 |
| 4 | `home.networks.heading` | 內建 12 條網絡，亦可以自己加 | mistranslation (stale fact) | High | C-net-1: 24 networks (test failed) | 內置 24 條網絡，亦可自行加入 |
| 5 | `home.networks.body` | …冇合約嘅話，可以用<a biubiu.tools>鏈部署工具</a>部署 | technical | Medium | href is not the English one; claimed every missing contract can be deployed | …<a href="/chain-setup">鏈設定</a>會列出缺少哪些，並部署任何人都能部署的那些 |
| 6 | `home.meta.description` / `ogDescription` / `organization` | …唔使硬件錢包，亦冇任何綁定。你畀錢買嘅係方便… | mistranslation | Medium | old claims. Omits the unmodified Safe and security keys, and "no hardware wallet needed" reads against security-key support | rewritten to the reference |
| 7 | `home.hero.facts[3]` | 就算 Vela 有一日停咗，你一樣入到自己個錢包 / 應用、中繼同每一個後端服務，你都可以自己跑 | mistranslation | High | overclaims. Passkeys stay tied to getvela.app and the relay needs a code change (C-rp-1) | Vela 為你的錢包運行的一切都是開源的，你可以自行運行。/ 要運行甚麼、沒有 getvela.app 時還有甚麼可用，以及甚麼仍然依賴我們 |
| 8 | `home.hero.facts[2].link` | Bybit 15 億美元就係咁樣冇咗 | mistranslation | Low | drops "and what Vela does about it" | Bybit 如何損失 15 億美元，以及 Vela 如何應對 |
| 9 | `home.why.p1` | <a account.base.app>…三樣嘢始終避唔開… | technical / mistranslation | Medium | href changed (test failed); missing the closed signing service and "no published way back" | new href; four walls, as in the reference |
| 10 | `home.tradeoffs.items[0].body` | …費用包括鏈上成本同中繼服務費，喺你簽名嗰陣就定咗。你可以喺設定入面換中繼器… | mistranslation | High | understates the fee. Missing 3× reserved gas, "often ten times or more", the first-send surcharge and the $0.01 minimum (C-fee-1) | the full formula; 「自行運行一個」 now links `/docs/self-hosting#relay` |
| 11 | `home.tradeoffs.items[1].body` | 開錢包嗰陣可以設幾把鑰匙 | mistranslation | High | C-keys-1: missing "up to seven" and "can't be changed afterwards" | 建立錢包時，你最多可以選七把鑰匙，之後不能更改。… |
| 12 | `home.tradeoffs.items[2]` | 你倚靠嘅係審計過嘅 Safe 合約——但審計唔等於保證 | mistranslation | High | C-audit-1: trade-off #3 must say Vela's own code is unaudited and nothing is scheduled | 合約經過審計，Vela 自己的程式碼沒有 / …沒有經過第三方審計，目前亦沒有排期 |
| 13 | `home.compare.rows` | 12 rows; 多一重檢查：一個獨立、可以自己部署嘅頁面或者擴充功能 | technical / mistranslation | High | array length 12 ≠ 13 (the "Adding a key later" row was missing); the extra-check row implied a usable check (C-signpage-1) | 13 rows; 一個獨立簽名頁，已經完成，但尚未接入 App |
| 14 | `home.compare.rows[10]` | 完整自行部署：應用、中繼同後端服務，全部你都跑得到 | mistranslation | High | omits the gaps and the getvela.app binding (C-rp-1) | App 和後端服務，但有自行架設指南所列的缺口；通行密鑰仍綁定 getvela.app |
| 15 | `home.compare.rows[11]` (+ MetaMask, Base cells) | 開源：應用、中繼同後端服務 / 錢包客戶端 / SDK；部分帳戶服務唔開源 | mistranslation | Medium | licence facts (C-lic-1) and MetaMask's non-commercial licence missing | 原始碼：…除索引外均採用 MIT 授權，索引的授權條款尚待補上 / 公開，但授權條款只允許非商業用途 |
| 16 | `home.pricing.cards` | 桌面同手機應用：自己編譯免費 / 喺應用商店下載：一次過買斷 | mistranslation | Medium | the desktop app is a free download; the store apps don't exist yet (C-plat-1) | three new cards, ending in 一次性購買 · 即將推出 |
| 17 | `home.seal.label` / `.verify` | 已經喺鏈上開咗嘅錢包 / 睇登記表 | terminology | Low | "registered", not "opened"; 登記表 is a form | 個錢包已在鏈上登記 / 查看註冊表 |
| 18 | `home.faq.items[0].a` | 一部用 Face ID 或者指紋解鎖到嘅機，又或者一把 USB/NFC 安全金鑰 | mistranslation | High | a wallet can't be created on one unsynced security key; you need two | …如果只用安全密鑰，就需要兩把… |
| 19 | `home.faq.items[1].a` | iOS、Android、桌面應用同 Vela 瀏覽器擴充功能都用得 | mistranslation | Medium | C-dapp-1: missing "the web wallet doesn't connect"; desktop is macOS and Windows only | …網頁版錢包不會連接 dApp |
| 20 | `home.faq.items[4].a` | 如果唔想靠 Apple 或者 Google，可以改用 USB/NFC 安全金鑰 | mistranslation | High | implies you can switch keys later. A key can't be removed, so move the funds to a new wallet (C-keys-1) | 請把資金轉到一個新錢包，並用不依賴該帳戶的鑰匙建立它… |
| 21 | `home.faq.items[5].a` | 淨係你把鑰匙先控制到個錢包。…你嘅公鑰、錢包地址同錢包名會公開喺鏈上 | mistranslation | Medium | omits "Vela writes the software that asks your keys to sign", that the services see your IP, and that the authenticator kind and key labels are public (C-reg-1) | rewritten to the reference |
| 22 | `home.faq.items[6].a` | …零依賴嘅清晰簽署擴充功能。你手上嘅鑰匙兩邊都用得… | mistranslation | High | the signing page is not a way in, because no app sends it requests (C-signpage-1); the relay code-change caveat was missing | the extension plus self-built apps (phone or security key); relay caveat added |
| 23 | `about.lede` | 背後冇一間面目模糊嘅公司：淨係有你睇得到嘅真代碼，同一個你搵得到嘅真人 | mistranslation | Medium | the reference names the maker, MONDAY LABS LTD, a UK company | Vela 由英國小公司 MONDAY LABS LTD 製作，全程公開開發… |
| 24 | `about.values[0]`, `[1]` | 你嘅鑰匙，你嘅幣…/ 你張臉，或者你隻手指 | mistranslation | Medium | missing "what we do control is the software you sign with"; face or fingerprint only (C-auth-1) | …我們確實掌控的，是你用來簽署的軟件… / …面容、指紋或 PIN… |
| 25 | `roadmap.upcoming[4]` (old) | …更多 EVM 網絡（包括冇 P-256 預編譯嘅鏈嘅簽名路徑），同埋一次針對 Vela 嘅 Safe + WebAuthn 整合嘅獨立安全審計 | cultural risk / mistranslation | High | promised an audit (forbidden by A02 FR-2) and a path for chains without P-256 (C-p256-1) | the array was replaced with the five new items; no audit promise anywhere |
| 26 | `roadmap.upcoming[3]` (old) | 喺 iOS 同 Android 上面，你嘅帳戶同網絡已經會跟住平台嘅備份走 | mistranslation | High | C-sync-1: account lists aren't synced; a new device rebuilds them from the registry | removed with the old array |
| 27 | `roadmap.shipped` | 7 old items | technical | Medium | the English is now 10 new items | replaced, 10 items |
| 28 | `getStarted` (`meta.description`, `lede`, `web.blurb`, `desktop.stores`, `fundingNote`) | Mac App Store · Microsoft Store; no dApp note; no limit on self-built phone apps | mistranslation | Medium | C-plat-1, C-rp-1 | rewritten; `stores` is Microsoft Store; the web card says to use the extension for dApps; a self-built phone app signs with another phone or a security key |
| 29 | `chrome.nav.alpha` | 測試版 | mistranslation | Low | 測試版 reads as "beta" | Alpha 版 |
| 30 | `introduction.md` | 12 條網絡；靠嘅係你張臉或者你隻手指；[安裝 Vela]——佢喺瀏覽器行，唔使下載 | mistranslation | High | C-net-1, C-auth-1, C-plat-1 | rewritten, with the new answer table |
| 31 | `install.md` | 冇嘢要下載，亦唔使經任何應用程式商店…原生手機應用即將推出; no dApps section | mistranslation / technical | High | the extension and desktop app are downloads (C-plat-1); the `dapps` anchor was missing (test failed) | rewritten; `<span id="dapps">` added |
| 32 | `create-wallet.md` | Vela 由你通行密鑰嘅公鑰推導出地址; a single-passkey flow | mistranslation / technical | High | C-addr-1 (all keys), C-keys-1 (up to seven, fixed); the `what-is-public` anchor was missing (test failed) | rewritten; anchor added |
| 33 | `networks-and-fees.md` | 12 條網絡；gas 帳戶啟用按金；「呢度冇速度選擇器」 | mistranslation | High | C-net-1, C-fee-1, C-fee-2, C-fee-3 | rewritten: the 24-network table, the fee formula, treasury top-up, speeds |
| 34 | `passkeys.md` | 淨係喺你刷完臉或者撳完指紋之後; only iCloud and Google | mistranslation | Medium | C-auth-1, C-keys-2 | rewritten, with the key-kind table |
| 35 | `recovery.md` | 用同一個 iCloud 或者 Google 帳戶登入…平台提供已同步嘅通行密鑰；索引提供對應嘅帳戶 | mistranslation | High | C-sync-1: sign in with any one key; registry fallback; a multi-key wallet needs the record | rewritten |
| 36 | `signers.md` | 喺開錢包嗰陣註冊一把 YubiKey，然後用佢簽 | mistranslation | Medium | one unsynced key can't create a wallet; you need two security keys | rewritten, with the per-app support table |
| 37 | `clear-signing.md` | 無限授權會俾人擋住…拒絕發出任何仲係無限嘅授權 | mistranslation | High | C-approve-1: signed permits and large finite approvals aren't capped; C-clear-1: "verified" isn't cryptographic | rewritten |
| 38 | `bybit-attack.md` | 狀態：已經做完並測過，未部署。上線之後佢係可選嘅 | mistranslation | High | no Vela app sends requests to it yet (C-signpage-1); the self-call gap (`enableModule` and others) was missing | rewritten |
| 39 | `account-contract.md` | …任何兼容 Safe 嘅介面都驅動到佢 | mistranslation | High | C-safeui-1: Safe tools can read your account; signing needs software that can get a getvela.app signature | rewritten |
| 40 | `security-audits.md` | Tempo 係我哋十二條內置網絡之一…; no Certora or Nethermind reviews, known issues or gaps | mistranslation | High | C-net-1, plus the current audit content | rewritten; addresses checked character by character against English |
| 41 | `whitepaper.md` | 應用同全部四項後端服務以 MIT 授權；gas 帳戶…唔退還嘅按金；冇隱藏加價；12 條網絡 | mistranslation | High | C-lic-1, C-fee-1, C-fee-2, C-net-1 | rewritten |
| 42 | `faq.md` | 內置 12 條 EVM 網絡；唔退還嘅按金去啟用佢個 gas 中繼帳戶；四項後端服務都以 MIT 授權 | mistranslation | High | C-net-1, C-fee-2, C-lic-1 | rewritten |
| 43 | `why-vela.md` | 做成可以自己部署，係為咗令你個錢包永遠唔使靠我哋呢間公司仲喺唔喺線; the callout linked `/docs/security-audits` | mistranslation | Medium | overclaim: the passkey-domain limit was missing; the callout lacked "the app decides what your key is asked to sign" and linked the wrong page | rewritten; the callout links the account contract and the threat model |
| 44 | `self-hosting.md` | (missing) | technical | High | eight pages link to it and its six anchors | created, with all six `<span id>` anchors |
| 45 | `clear-signing-self-host.md` | register only (開唔到鎖匙…) | terminology | Low | facts already matched | rewritten in the written register |

Totals fixed: **High 24**, **Medium 17**, Low 4.

## Open items

- **Register change needs a native read.** The locale now departs from 059's spoken-Cantonese choice, for
  the reasons given above. If the founder prefers spoken Cantonese for the site, the change can be
  reversed, but the app corpus would then have to follow for the two to match.
- **The app's zh-HK labels are still partly spoken Cantonese.** Examples: 「呢部機」 for "This device",
  「手機或者平板」, 「鎖匙」 for Keys, and 「自行部署指南 →」 in Service Endpoints where the site says 自行架設指南.
  The docs name those controls in written form (本裝置、手機或平板電腦) and quote only labels that are already
  written (收款、發送、重設為預設值、較慢／標準／超快). The speed label 「超快」 over-translates "Fast"; the docs
  quote it because users see it. These are outside this task's files.
- `review.json` still lists zh-HK as `drafted`. No native Hong Kong reader has read this text yet, and
  this task does not edit that file.

## Result

reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the same-day en + zh revision (fee wording, configurable relay chain directory, hero subtitle,
facts #3 and #4) into zh-HK, in written Hong Kong Chinese (書面語). Terms unchanged from the table above:
中繼、通行密鑰、鏈數據、鏈目錄、自行運行／自行架設、確認畫面、簽署. Each changed string checked on the five
axes; no open High or Medium.

| String / section | Change | Severity of fixes beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | 簽署在你的裝置上完成。通行密鑰的私鑰絕不會交給 Vela。 | — | the claim is now about the private key, as in en |
| `home.hero.facts[2]` | 所見即所簽：在你確認之前，Vela 會解碼出你實際要簽署的那筆交易。 / link: …以及 Vela 如何讓你看清要簽署的內容 | — | 所見即所簽 is the established Chinese term for "what you see is what you sign" |
| `home.hero.facts[3]` | term = 059 string, **fixed**: 就算 Vela 有一日停咗，你一樣入到自己個錢包。 → 即使 Vela 停止服務，你仍然可以存取自己的錢包。 / link: 如果 Vela 消失，如何繼續使用你的錢包 | Medium | 059 text was spoken Cantonese; this locale is written Chinese since 080 (finding 1). Meaning kept. 存取 as in this locale's whitepaper (獨立存取錢包); link echoes the whitepaper heading 如果 Vela 消失 |
| `home.tradeoffs.items[0].body` | new fee paragraph (one fee, to the relay, Vela's by default; 3 × reserved gas at the chosen speed; $0.01 minimum; `#fee` link 計算方法); "十倍或以上" removed | Low | last clause 運行它的人 → 運行它的一方 (the operator can be an organisation), as in zh |
| `home.faq.items[6].a` | code-change clause replaced by 每一項你都可以自行運行——中繼、公鑰索引、鏈數據和匯率服務 | — | 匯率服務 to match the self-hosting section name |
| `roadmap.upcoming[1].body` | dropped 中繼也應該能從你自己的伺服器讀取鏈數據 | — | |
| networks-and-fees.md | `<span id="fee"></span>` under 手續費是多少; ten-times paragraph rewritten (first send also deploys the wallet; 你無須猜測); new **手續費歸誰。** paragraph linking `/zh-HK/docs/self-hosting#relay` | — | |
| faq.md | cost bullet (除非你把錢包指向另一個中繼或自行運行一個，否則就是 Vela 的中繼; `#fee` link); shutdown answer loses the relay code-change parenthesis | — | |
| whitepaper.md | intro clause removed; Fees bullet rewritten + new bullet on who gets the fee; 如果 Vela 消失 clause removed | — | |
| self-hosting.md | intro limit removed; two code comments (`VELA_RELAY_CHAIN_DIRECTORY_URL`); 須知 bullet rewritten (default, variable, Sept 2026, older builds); chain-data paragraph (variable + one-hour cache 快取); relay line removed from 還有甚麼指向 Vela | — | |

Checks: JSON parses, `_fingerprints` untouched; `id="fee"` ×1; no 十倍 in networks-and-fees / faq / whitepaper;
`rpc.rs` ×0; no unprefixed `](/docs` links.
