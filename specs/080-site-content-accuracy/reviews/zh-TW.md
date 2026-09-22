# zh-TW — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), native zh-TW localizer · Method: single-string locale review, whole pages

Scope: `app-web/getvela.app/src/lib/i18n/messages/zh-TW.json` (every key the brief lists, plus
defects found elsewhere) and all 17 docs under `src/content/docs/zh-TW/` (`self-hosting.md` is new).
Every doc was rewritten from the current English and Simplified Chinese; the old zh-TW text was
used only for terminology where the meaning still matched. It was not a character conversion of
`zh`: vocabulary, UI labels and sentence shapes are the ones a Taiwanese product team writes.

## Terminology and register

| Concept | zh-TW | Note |
| --- | --- | --- |
| key (any signing credential) | 金鑰 | umbrella term; matches the app's own zh-TW labels (金鑰) and the 059 fourth pass |
| passkey | 密碼金鑰 | 059 choice, kept (see Open items: the app UI says 通行密鑰) |
| security key | 安全金鑰 / 硬體安全金鑰 | Google's zh-TW term |
| relay | 中繼 | 059 sixth pass, not 中繼器 |
| registry | 註冊表 (verb 註冊) | the app already says 「暫時連不上註冊表」; the old site text mixed 鏈上登記 / 登記表 / 註冊 |
| public-key index | 公鑰索引 | |
| self-hosting | 自架 / 自己架設; guide 自架指南 | existing footer term; 自行部署 was mixed in and removed |
| signing page | 簽署頁 | old text mixed 簽名頁 and 簽署頁 (sidebar title fixed too) |
| sign / signature | 簽署 / 簽章 | Taiwan usage (數位簽章); same pair as the app |
| clear signing / blind signing | 清晰簽署 / 盲簽 | as the app |
| descriptor | 描述檔 | as the app; old docs had 描述符 |
| audit | 稽核 | 059 eighth pass (no 審計 anywhere) |
| address / device / account | 位址 / 裝置 / 帳戶 (on-chain) vs 帳號 (Apple, Google) | Taiwan distinction kept |
| other Taiwan vocabulary | 軟體、網路、設定、資訊、檔案、資料夾、登入、程式碼、原始碼、伺服器、預設、支援、呼叫、函式、QR 碼、試算表、幣別 | no mainland forms (信息、默認、二維碼、代碼、服務器…) |

UI labels are quoted with 「」 exactly as Taiwanese users see them: the Vela app's zh-TW corpus
(`assets/i18n/zh-TW.json`) — 收款、發送、設定 → 網路、設定 → 進階 → 服務端點、重設為預設值、
較慢／標準／超快、法幣匯率; Chrome zh-TW — 開發人員模式、載入未封裝項目; Windows SmartScreen zh-TW —
Windows 已保護您的電腦、其他資訊、仍要執行.

Register: plain 你, unchanged from 059. Product, contract and network names, EIP/ERC numbers,
commands, addresses, file paths and code stay in English; code comments are translated.
Full-width punctuation throughout; 「」 for quotes; bold markers placed so CommonMark still
renders them next to CJK punctuation (checked mechanically, 0 bad delimiters).

## Findings

Findings are against the OLD zh-TW text. Everything listed was fixed in this pass.

### Catalog (`zh-TW.json`)

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | `home.networks.heading` / `.body` | 內建 12 條網路；缺少合約時用鏈部署工具部署 | mistranslation (stale fact) | High | 24 networks (C-net-1); chain setup deploys only what anyone can deploy | 內建 24 條網路…部署任何人都能部署的那些 |
| C2 | `home.faq.items[0].a` | 一台能用 Face ID 或指紋解鎖的裝置，或者一把 USB/NFC 安全金鑰 | mistranslation | High | a wallet can't rest on one unsynced security key (two needed); seven-key limit missing | 如果只用安全金鑰，需要兩把…最多七把 |
| C3 | `home.faq.items[1].a` | iOS、Android、桌面應用程式和 Vela 瀏覽器擴充功能都支援 | mistranslation | High | implies Linux desktop; omits that the web wallet doesn't connect (C-dapp-1) | 桌面版（macOS、Windows）…網頁錢包不會連接 dApp |
| C4 | `home.faq.items[4].a` | 如果不想依賴 Apple 或 Google，可以改用 USB/NFC 安全金鑰 | mistranslation | High | "switch to" a key is impossible after creation; omits that a key can't be removed and funds must move | 金鑰無法移除。請把資金轉到一個新錢包… |
| C5 | `home.faq.items[5].a` | Vela 不能轉走也不能凍結…你的公鑰、錢包位址和錢包名稱會公開 | mistranslation (softened) | High | drops "Vela writes the software that asks your keys to sign"; public list incomplete (C-reg-1); no IP/transaction visibility | full en+zh meaning restored |
| C6 | `home.faq.items[6].a` | 也可以用…零依賴的清晰簽署擴充功能；後端服務也可以獨立運作 | mistranslation (false) | High | the signing page is unpublished and can't operate a wallet (C-signpage-1); relay code-change caveat missing | 擴充功能或自行建置的應用程式；中繼需要改一處程式碼 |
| C7 | `home.tradeoffs.items[0]` | 費用包含鏈上成本與中繼服務費…在設定裡更換中繼 | mistranslation (stale fact) | High | fee model wrong (C-fee-1: 3× reserved gas, often 10×+); link must go to `/docs/self-hosting#relay` | 三倍預留 gas、常常十倍以上、最低 0.01 美元 |
| C8 | `home.tradeoffs.items[1].body` | 建立錢包時可以設定多把金鑰（no full stop, no limit） | mistranslation | Medium | missing "up to seven, can't be changed afterwards" (C-keys-1) | 最多可以選七把金鑰，之後不能再更改 |
| C9 | `home.tradeoffs.items[2]` | 你信任的是稽核過的 Safe 合約——而稽核不是保證 | mistranslation (omission) | High | the trade-off must say Vela's own code is unaudited and none is scheduled (C-audit-1) | 合約經過稽核，Vela 自己的程式碼沒有 |
| C10 | `home.compare.rows` extra check / full self-hosting | 可透過獨立、自架的頁面或擴充功能核對；應用程式、中繼與後端服務均可自行運行 | mistranslation (overclaim) | High | signing page not connected (C-signpage-1); self-hosting has gaps and passkeys stay tied to getvela.app (C-rp-1) | 已完成，但還沒接上應用程式；…密碼金鑰仍綁定 getvela.app |
| C11 | `home.compare.rows` (structure, competitors) | 12 rows; 開源範圍; MetaMask EOA / 不提供代付 | mistranslation (stale) | Medium | 13 rows with "Adding a key later"; "Source code" licence facts (C-lic-1); competitor cells updated in en | 13 rows, 原始碼 row, 事後新增金鑰 row |
| C12 | `home.meta.description` / `ogDescription` / `organization` | 支援 ETH 與 ERC-20…不需要硬體錢包，也沒有任何綁定 | mistranslation (stale) | Medium | "no hardware key / no lock-in" retired; security keys are supported | Safe-based, passkeys or security keys |
| C13 | `home.hero.facts[3]` | 即使 Vela 停止服務，你仍然能存取自己的錢包；每一項後端服務你都能自己跑 | mistranslation (stale, overclaim) | Medium | new claim is "open source, you can run it" plus what still depends on Vela | 你也可以自己架設 / 哪些仍然依賴我們 |
| C14 | `home.seal.label` / `.verify` | 已在鏈上建立的錢包；查看鏈上登記 | mistranslation | Medium | the counter counts registrations, not deployed contracts | 已在鏈上註冊的錢包；查看註冊表 |
| C15 | `home.why.p1` | account.base.app; 復原金鑰由瀏覽器產生；服務沒了，錢包也跟著沒了 | mistranslation (stale) | Medium | en now: recovery phrase created on a website, closed signing service, no published path; new href | rewritten, href updated |
| C16 | `home.pricing.cards` | 桌面與手機應用・自己編譯免費；從應用商店下載・一次性買斷 | mistranslation (stale fact) | High | desktop is a free download; store apps not available yet (C-plat-1) | 網頁版、擴充功能和桌面版免費；…一次性買斷 · 即將推出 |
| C17 | `about.values[1].body` | 加密資產遺失最主要的原因…你的臉或指紋 | cultural risk / mistranslation | Medium | unsourced superlative; face/fingerprint-only contradicts C-auth-1 | 裝置或安全金鑰保管…臉部辨識、指紋或 PIN 碼 |
| C18 | `about.lede`, `about.values[0]`, `[2]` | 背後沒有一家面目模糊的公司；你的金鑰，你的幣——這不是一句口號 | cultural risk (slogan) / omission | Medium | en names MONDAY LABS LTD and says Vela controls the signing software | plain statements, company named |
| C19 | `roadmap.upcoming` | 通訊錄、不用手機的桌面 dApp 連線、無 P-256 鏈的簽署路徑、對整合做一次獨立安全稽核 | mistranslation (false promises) | High | an audit may never be promised (A02 FR-2); P-256 path contradicts C-p256-1; others shipped | the 5 new items |
| C20 | `roadmap.shipped` | 7 old items (以支付為中心的首頁…) | mistranslation (stale) | Medium | replaced by the 10 new items | the 10 new items |
| C21 | `getStarted.platforms.web.blurb` | 不用安裝，也不用更新…用密碼金鑰驗證身分 | omission | Medium | web wallet doesn't connect to dApps; confirm with any of your keys | 要連接 dApp，請用擴充功能 |
| C22 | `getStarted.platforms.desktop.stores` | Mac App Store · Microsoft Store | mistranslation | Medium | Mac App Store is not a channel | Microsoft Store |
| C23 | `getStarted.fundingNote` | 同一個應用程式，不收一分錢 | mistranslation (softened) | High | a self-built phone app can't use the phone's own passkey (C-rp-1) | 自行建置的手機應用程式是用另一支手機或安全金鑰簽署 |
| C24 | `getStarted.meta.description` / `lede` | 桌面版、行動版和擴充功能都由同一份程式碼建置 | mistranslation (stale) | Low | en: extension and desktop downloadable now, phone on the way | updated |
| C25 | `home.faq.items[2].a` | Google 密碼管理程式 | terminology | Low | Google's zh-TW product name is 密碼管理工具 (the docs already used it) | Google 密碼管理工具 |
| C26 | `chrome.nav.alpha` | 測試版 | terminology | Low | reads as "beta/test build"; every doc says alpha | Alpha |
| C27 | `chrome.docs.titles["clear-signing-self-host"]` | 自己部署簽名頁 | terminology | Low | inconsistent with 自架 / 簽署頁 | 自己架設簽署頁 |
| C28 | `home.hero.facts[2].link` | Bybit 的 15 億美元就是這樣丟的 | mistranslation (stale) | Low | en adds "and what Vela does about it" | 以及 Vela 怎麼應對 |

`chrome.docs.groups.keys` / `selfHost`, `titles["self-hosting"]`, `footer.links.selfHosting` and
`chrome.docs.ui` drafts were reviewed and kept (金鑰與復原 / 自己架設 / 自架指南 / 瀏覽文件…下一頁 →).

### Docs (`src/content/docs/zh-TW/`)

| # | File | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| D1 | introduction, networks-and-fees, faq, whitepaper, security-audits | 12 條網路 | mistranslation (stale fact) | High | C-net-1 | 24 條網路, full list |
| D2 | networks-and-fees, faq, whitepaper | 每條網路一筆不可退還的押金來啟用 gas 帳戶 | mistranslation (false) | High | mechanism no longer exists (C-fee-2) | treasury top-up: optional, non-refundable, doesn't pay your fee |
| D3 | networks-and-fees, whitepaper | 這裡沒有速度選擇器；中繼是唯一價格來源；沒有隱藏加價 | mistranslation (false) | High | speed picker exists (C-fee-3); fee = 3× reserved gas at the higher price (C-fee-1) | fee formula, 「較慢／標準／超快」 |
| D4 | create-wallet, recovery, passkeys, whitepaper, introduction | 用一把密碼金鑰；由你密碼金鑰的公鑰推導出位址 | mistranslation (stale fact) | High | 1–7 keys; address from all founding keys (C-addr-1, C-keys-1) | rewritten |
| D5 | passkeys, create-wallet, introduction | 私鑰由 iCloud 鑰匙圈或 Google 密碼管理工具保管；靠你的臉或指紋 | mistranslation | High | three kinds of key (C-keys-2); PIN / security-key touch too (C-auth-1) | key-kind table, all checks named |
| D6 | recovery | 用同一個 iCloud 或 Google 帳號登入 as the only route | mistranslation (stale) | High | any one key signs in; index → Gnosis → Ethereum registry fallback (C-sync-1) | rewritten |
| D7 | account-contract | 任何相容 Safe 的介面都能驅動它 | mistranslation (false) | High | tools can read/build; signing needs a getvela.app signature (C-safeui-1) | rewritten |
| D8 | whitepaper | Vela 伺服器被攻陷——不會取得任何簽署能力，只是服務降級 | mistranslation (false) | High | app delivery can present a malicious transaction; services have real influence | threat model split into backend vs delivery vs domain |
| D9 | whitepaper | 網域遺失的一般使用者復原路徑仍未完成 | mistranslation (stale) | High | extension and self-built apps keep working (C-rp-1) | 「如果 Vela 消失了」 rewritten |
| D10 | send-and-receive, clear-signing | Vela 會把無限授權改寫成有限金額／無限授權會被擋下 | mistranslation | High | you reduce it; permits and large finite approvals are only cautioned (C-approve-1) | Callout rewritten with the permit caveat |
| D11 | bybit-attack | 自己跑一份是唯一不需要信任任何人的答案；沒有我們能升級的合約 | mistranslation (overclaim) | High | you still trust code you build; owner-signed `delegatecall` still exists; self-call gap (`enableModule`…) was missing | rewritten with both limits |
| D12 | clear-signing-self-host | no status line; 不會自己發任何網路請求 | omission / mistranslation | High | not published, no app sends requests (C-signpage-1); it fetches token logos | status line added, logo request stated |
| D13 | faq | 錢包和四項後端服務都以 MIT 授權 | mistranslation (false) | High | index has no licence file (C-lic-1) | licence facts per service |
| D14 | faq | Vela 保存你的公鑰和名字…看不到 | omission | Medium | what each service sees is listed in en | full list |
| D15 | install | 在瀏覽器裡跑、不用下載；原生行動應用即將推出並會原封不動跟過去；Android 9+ | mistranslation (stale fact) | High | extension and desktop exist; phone apps paid and not yet in stores (C-plat-1); device table outdated | rewritten; `dapps` anchor added |
| D16 | why-vela | 把第一把金鑰設成硬體安全金鑰；可自行部署…永遠不必依賴我們在不在線上 | mistranslation (overclaim) | High | security-keys-only needs two; the domain limit must be stated | 只用硬體安全金鑰——要兩把；網域限制 |
| D17 | security-audits | no Certora M-01 on the 4337 module; no "gaps" section; interception 「幾乎沒有機會」 | omission / softened | High | unfixed medium finding and Vela's own gaps must be disclosed | full page rewritten |
| D18 | send-and-receive | no split/sweep, name lookup, fee coin & speed sections | omission | Medium | sections added in en | added |
| D19 | signers | no app × key-kind table, no compromise section; 鏈上的 WebAuthn 驗證器 | omission / technical | Medium | shared signer vs per-key signer; what to do if a key leaks | added |
| D20 | why-vela | Callout linked 「Safe 智慧帳戶」 to `/docs/security-audits` | technical | Low | the en links `/docs/account-contract` | fixed |
| D21 | clear-signing, clear-signing-self-host, bybit-attack | 描述符 / 簽名頁 / 簽署頁 mixed | terminology | Low | one term each | 描述檔, 簽署頁 |

Links: every internal docs link carries `/zh-TW/`; `/blog…`, `/privacy`, `/terms`, `/registry`
stay unprefixed; all anchors present (`self-hosting`: 6, `install`: `dapps`, `create-wallet`:
`what-is-public`).

## Open items

1. **passkey: site 密碼金鑰, app 通行密鑰.** The site keeps 059's 密碼金鑰 as instructed, but the
   app's own zh-TW corpus says 通行密鑰 (e.g. the Service Endpoints field 「通行密鑰索引」). The
   self-hosting guide names that field 「密碼金鑰索引（畫面上顯示為「通行密鑰索引」）」 so a reader can
   find it. One of the two should move; that is a product-wide terminology decision, not a
   translation fix.
2. **「超快」 for the default speed.** The app labels *fast* as 超快, which is stronger than the
   English; the docs quote the app label (networks-and-fees ×2, send-and-receive ×1) so the text
   matches the screen. If the app label becomes 快速, those three follow.
3. **自架指南 vs 自行部署指南.** The app's link to this page reads 「自行部署指南 →」; the site title is
   the existing 自架指南. Same page, two names; left as the site's term.
4. **`about.team.bio`.** `zh` says 一個人做 (alone); the English does not claim that. zh-TW follows
   the English (no solo claim). Worth aligning `zh` or confirming the claim.

## Result

reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the same-day en + zh revision (fee wording, configurable relay chain directory, hero subtitle,
facts #3 and #4) into zh-TW. Terms unchanged from the table above: 中繼、密碼金鑰、鏈資料、鏈目錄、自架／架設、
確認頁、簽署. Each changed string checked on the five axes; no open High or Medium.

| String / section | Change | Severity of fixes beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | 簽署都在你的裝置上完成。密碼金鑰的私鑰絕不會交給 Vela。 | — | the claim is now about the private key, as in en |
| `home.hero.facts[2]` | 所見即所簽：在你確認之前，Vela 會解碼出你實際要簽署的那筆交易。 / link: …以及 Vela 怎麼讓你看清楚要簽署的是什麼 | — | 所見即所簽 is the established Chinese term for "what you see is what you sign" |
| `home.hero.facts[3]` | term = 059 string, **kept**: 即使 Vela 停止服務，你仍然能存取自己的錢包。 / link: 如果 Vela 不在了，要怎麼繼續使用你的錢包 | — | 059 term is accurate and natural (存取 is the standard Taiwan word for "access", as in Google zh-TW) |
| `home.tradeoffs.items[0].body` | new fee paragraph (one fee, to the relay, Vela's by default; 3 × reserved gas at the chosen speed; $0.01 minimum; `#fee` link); "often ten times or more" removed | Low | last clause 手續費就歸架設它的人 → 營運它的一方: "runs" means operates, matching the networks-and-fees heading 中繼由誰營運 |
| `home.faq.items[6].a` | code-change clause replaced by 每一項你都能自己架設——中繼、公鑰索引、鏈資料和匯率服務 | — | 匯率服務 (not bare 匯率) to match the self-hosting section name |
| `roadmap.upcoming[1].body` | dropped 中繼也應該能從你自己的伺服器讀取鏈資料 | — | |
| networks-and-fees.md | `<span id="fee"></span>` under 手續費是多少; ten-times paragraph rewritten (first send also deploys the wallet; 你不必用猜的); new **手續費歸誰。** paragraph linking `/zh-TW/docs/self-hosting#relay` | — | bold closes before a space, as elsewhere in this locale |
| faq.md | cost bullet (Vela's relay by default, or another / your own; `#fee` link); shutdown answer loses the relay code-change parenthesis | — | |
| whitepaper.md | intro clause removed; Fees bullet rewritten + new bullet on who gets the fee; 如果 Vela 消失了 clause removed | — | |
| self-hosting.md | intro limit removed; two code comments (`VELA_RELAY_CHAIN_DIRECTORY_URL`); 你該知道的 bullet rewritten (default, variable, Sept 2026, older builds); chain-data paragraph (variable + one-hour cache 快取); relay line removed from 還有哪些指向 Vela | — | |

Checks: JSON parses, `_fingerprints` untouched; `id="fee"` ×1; no 十倍 in networks-and-fees / faq / whitepaper;
`rpc.rs` ×0; no unprefixed `](/docs` links.
