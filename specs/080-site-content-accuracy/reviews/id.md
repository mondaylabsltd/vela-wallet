# id — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), Indonesian localizer · Method: single-string locale review, whole pages

Scope: every key the brief lists in `id.json`, the chrome drafts, plus the defects
found in keys it did not list; all 17 docs rewritten from the current English and
Chinese (`self-hosting.md` created). The old Indonesian docs were translated from an
English that predates spec 080, so most findings are accuracy: an Indonesian reader
was being told things the product no longer does (12 networks, a gas-account
deposit, "nothing to download"), or never did (an audit on the roadmap, a relay
that rewrites unlimited approvals).

## Terminology and register

| Concept | id | Note |
| --- | --- | --- |
| key (any signing credential) | kunci | umbrella term; the wallet's own Keys screen is *Kunci* |
| passkey | passkey | 059 choice, kept; what Indonesian tech press and the wallet UI write |
| security key / hardware security key | kunci keamanan / kunci keamanan fisik | *kunci keamanan* is the wallet UI term (*Kunci keamanan USB*); *fisik* only where English says "hardware", matching `hero.facts[1].link` |
| signer (a Safe owner) | kunci penanda tangan | as the sidebar title; the contract noun stays *signer* (*kontrak signer*, *shared signer*) |
| relay | relay | kept in English, as the product and the Settings label *Vela Relay* do; *relayer* only for the relay's own sender addresses |
| registry | registri / kontrak registri | the wallet UI word (*Registri tidak dapat dijangkau*) |
| public-key index | indeks kunci publik | the service; the Settings field is quoted by its UI label, **Indeks passkey** |
| self-hosting | hosting sendiri (menjalankan sendiri) | the site's existing term (sidebar, footer); the wallet UI button says *Panduan self-hosting* — see Open items |
| signing page | halaman tanda tangan | as the existing sidebar title |
| clear signing / blind signing | clear signing / tanda tangan buta | clear signing kept (059 and the wallet UI); *Tanda tangan buta* is the wallet UI label |
| decode | mendekode / didekode | the wallet UI's own verb (*Didekode dari…*); replaces *menerjemahkan*, which read as "translate" |
| self-custodial | non-kustodial | **docs changed** from *swakelola* (a procurement word, rare in Indonesian crypto writing) to *non-kustodial*, which `about.values[0].title` already used |
| seed / recovery phrase | frasa pemulihan | the wallet UI term; the footer's leftover *seed phrase* aligned |
| password manager | pengelola kata sandi | wallet UI term |
| iCloud Keychain / Google Password Manager | Rantai Kunci iCloud / Pengelola Sandi Google | Apple's and Google's own Indonesian names, as the reader sees them in Settings |
| chain / network | chain / jaringan | *chain* as in the Settings label *Indeks data chain*; *rantai* (old docs) dropped |
| deploy / on-chain / open source | men-deploy, di-deploy / on-chain / open source | the forms Indonesian developer writing uses; *dipasang* (old docs) confused deploying a contract with installing an app |
| alpha | alfa (prose) | the nav pill stays *Alpha* |

Register: formal *Anda*, as recorded in 059, in plain documentation Indonesian —
*bisa*, *kalau*, *pakai* rather than stiff *dapat/apabila/menggunakan* everywhere,
no slang. UI labels are quoted as the Indonesian builds show them: the wallet corpus
(*Buat Dompet*, *Perangkat ini*, *Ponsel atau tablet*, *Kunci keamanan USB*,
*Terima*/*Kirim*, *Lambat*/*Standar*/*Cepat*, *Pengaturan → Lanjutan → Endpoint
Layanan*, *Setel Ulang ke Bawaan*, *Kurs fiat*, *Terverifikasi*, *Ketentuan
Layanan*, *Kebijakan Privasi*), Chrome (*Mode developer*, *Muat yang belum dibuka*,
with the English label in parentheses) and Windows SmartScreen (*Windows melindungi
PC Anda*, *Info selengkapnya*, *Tetap jalankan*). Indonesian number format: $0,01,
140.000, 1,1 juta, $1,5 miliar. Contract names, EIP/ERC numbers, commands, file paths
and addresses as in English; comments in code blocks and the architecture diagram are
translated, as `zh` does. The status words around audits use *tidak diaudit* for bare
status and *belum pernah diaudit … dan tidak ada audit yang dijadwalkan* for Vela's
own code, so that *belum* ("not yet") never stands alone where it could read as a
promise.

## Findings

Catalog (`id.json`):

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `home.meta.description`, `organization` | "Tanda tangani dengan passkey — tanpa frasa pemulihan, **tanpa dompet perangkat keras**, tanpa terkunci pada siapa pun" | mistranslation (stale) | High | denies the security-key support C-keys-2 describes; "bisa Anda hosting sendiri" overclaims (C-rp-1) | passkey atau kunci keamanan; Safe yang tidak dimodifikasi; layanan open source |
| 2 | `home.meta.ogDescription` | "untuk ETH dan ERC-20, bisa di-hosting sendiri… Kalau mau, kompilasi sendiri" | mistranslation (stale) | Medium | no security keys, no Safe, self-hosting overclaim | from the reference |
| 3 | `home.hero.facts[3]` | "Kalau Vela berhenti beroperasi, Anda tetap bisa mengakses dompet Anda." / "Hosting sendiri aplikasinya, relay-nya, dan semua layanan pendukungnya" | mistranslation (stale) | High | a promise the passkey-domain limit contradicts (C-rp-1) | "Semua yang dijalankan Vela… bisa Anda jalankan sendiri" / what still depends on us |
| 4 | `home.hero.facts[2].link` | "…dan celah yang kami tutup" | mistranslation | Medium | claims the attack path is closed; the docs say mitigated, not eliminated | "dan apa yang Vela lakukan soal itu" |
| 5 | `home.why.p1` | href `account.base.app`; "kalau layanannya berhenti, dompetnya ikut berhenti" | technical + mistranslation | High | href differed from English (test failure); the complaint is now the closed signing service | new href, "(kini bagian dari Coinbase Wallet)", "layanan tanda tangan yang kodenya tidak terbuka" |
| 6 | `home.tradeoffs.items[0]` | "Biayanya mencakup ongkos on-chain dan biaya layanan relay"; GitHub link; "mengganti relayer di pengaturan" | mistranslation (stale) | High | hides C-fee-1 (3 × reserved gas, often 10× the real cost, $0,01 minimum) | full fee rule, `/docs/self-hosting#relay`, "mengarahkan dompet ke relay lain" |
| 7 | `home.tradeoffs.items[1].body` | "Anda bisa menetapkan beberapa kunci saat membuat dompet" (no full stop) | mistranslation (stale) | High | drops "up to seven" and "can't be changed afterwards" (C-keys-1) | "hingga tujuh kunci… tidak bisa diubah sesudahnya" |
| 8 | `home.tradeoffs.items[2]` | "Anda bersandar pada kontrak Safe yang sudah diaudit — dan audit bukan jaminan" | mistranslation (stale) | High | never says Vela's own code is unaudited, which the ledger requires of this card (C-audit-1) | "Kontraknya sudah diaudit, kode Vela sendiri tidak" + "tidak ada audit yang dijadwalkan"; *tidak*, not *belum*, in the title |
| 9 | `home.compare.rows` | 12 rows; "Passkey" only; MetaMask sponsored gas "Tidak tersedia"; "Aplikasi, relay, dan layanan pendukung, semuanya bisa Anda jalankan"; "Halaman atau ekstensi independen yang Anda hosting sendiri"; "Open source… Aplikasi, relay, dan layanan pendukung" | technical + mistranslation (stale) | High | array one row short (`id.home is not a fragment` failed); cells false against C-signpage-1, C-rp-1, C-lic-1 and MetaMask's own sponsorship | 13 rows from the reference, incl. "Menambah kunci belakangan" and "Kode sumber" |
| 10 | `home.pricing.cards` | "Aplikasi desktop & seluler — Gratis kalau dikompilasi sendiri"; "Dari toko aplikasi — Sekali beli" | mistranslation (stale) | Medium | desktop is a free download, not source-only; store apps are not out yet (C-plat-1) | three cards from the reference, "Sekali beli · segera hadir" |
| 11 | `home.networks.heading`, `.body` | "12 jaringan bawaan"; external biubiu.tools link; "Anda bisa memasangnya" | technical + mistranslation (stale) | High | 24 networks (C-net-1, test failure); link now `/chain-setup`, which deploys only what anyone can deploy | "24 jaringan bawaan"; *penyiapan chain* |
| 12 | `home.faq.items[0].a` | "Perangkat yang bisa dibuka dengan Face ID atau sidik jari, atau sebuah kunci keamanan USB/NFC" | mistranslation (stale) | High | one security key alone can't create a wallet (two are needed); face/fingerprint as the only check (C-auth-1); no "up to seven" | from the reference |
| 13 | `home.faq.items[1].a` | "Vela menaruh dompet langsung di dalam dApp… aplikasi iOS, Android, dan desktop" | mistranslation (stale) | Medium | omits that the web wallet doesn't connect and that desktop means macOS/Windows (C-dapp-1) | from the reference |
| 14 | `home.faq.items[4].a` | "Siapa pun yang bisa menjangkau passkey Anda yang tersinkron bisa saja menjangkau dompet Anda juga." | mistranslation (stale) | High | missing the action a compromised account needs: a key can't be removed, move funds to a new wallet | from the reference |
| 15 | `home.faq.items[5].a` | "Vela tidak bisa memindahkan atau membekukan dana Anda. Hanya kunci Anda yang mengendalikan dompet." / only "kunci publik, alamat, nama" public | mistranslation (stale) | High | drops "by itself" and "it writes the software that asks your keys to sign"; understates what is public and what services see (C-reg-1) | from the reference; *secara sepihak* for "by itself" |
| 16 | `home.faq.items[6].a` | "tanda tangani lewat browser: ekstensi Vela, atau ekstensi clear signing yang tanpa dependensi" | mistranslation (stale) | High | offers the signing page as a working route; it is unpublished and no app sends it requests (C-signpage-1) | extension + self-built apps; relay code-change caveat |
| 17 | `home.faq.items[2].a` | "Anda memulihkannya di perangkat baru"; "iCloud Keychain / Google Password Manager" | mistranslation + UI fit | Low | "can recover" lost its modal; OS names not as Indonesian Settings show them | "Anda bisa memulihkannya"; Rantai Kunci iCloud / Pengelola Sandi Google |
| 18 | `home.seal.label`, `.verify` | "dompet dibuat on-chain" / "Setiap dompet ada di on-chain" | mistranslation (stale) | Low | reference now says *registered*; "ada di on-chain" is also a calque | "dompet terdaftar on-chain" |
| 19 | `about.lede`, `team.bio` | "dompetnya, smart contract-nya, dan situs ini"; "Tidak ada perusahaan tanpa wajah…" | mistranslation (stale) | Medium | Vela writes no contract in the funds path (C-acct-1); company name and UK missing | MONDAY LABS LTD, apps + backend + site on GitHub |
| 20 | `about.values[0..2].body` | "Kunci Anda, koin Anda — bukan slogan…"; "passkey: wajah atau sidik jari Anda"; "Dompetnya terbuka di GitHub" | mistranslation (stale) | Medium | omits that Vela controls the signing software; face/fingerprint only (C-auth-1); services left out | from the reference |
| 21 | `roadmap.upcoming` | "…dan audit keamanan independen untuk integrasi Safe + WebAuthn milik Vela"; "jalur penandatanganan untuk rantai tanpa precompile P-256"; "akun dan jaringan Anda sudah ikut cadangan platform" | mistranslation (stale) | High | promises an audit (forbidden), a P-256-less path (C-p256-1) and account sync (C-sync-1) | the new 5-item array |
| 22 | `roadmap.shipped` | 7 items, incl. WalletPair-era "Alur penandatanganan dApp" | technical (stale) | High | English has 10 items (`id.roadmap is not a fragment` failed) | the new 10-item array |
| 23 | `getStarted.meta.description`, `lede` | "versi desktop, seluler, dan ekstensi browser yang dibangun dari kode yang sama"; "ke desktop, ponsel, atau bilah alat" | mistranslation (stale) | Medium | implies the phone apps are available (C-plat-1) | phones "sedang disiapkan" / "sebentar lagi" |
| 24 | `getStarted.platforms.web.blurb`, `desktop.stores`, `fundingNote` | "tidak ada yang perlu diperbarui… autentikasi dengan passkey Anda"; "Mac App Store · Microsoft Store"; "aplikasi yang sama, tanpa biaya" | mistranslation (stale) | Medium | no dApp caveat for web (C-dapp-1); not on the Mac App Store; a self-built phone app can't use the phone's passkey (C-rp-1) | from the reference |
| 25 | `getStarted.platforms.extension.blurb` | "membiarkan dApp berbicara langsung dengannya — tanpa langkah pemasangan pasangan" | unnatural | Low | "pemasangan pasangan" is a garbled calque of "pairing step"; "berbicara dengan" is a calque of "talk to" | "memungkinkan dApp terhubung langsung ke dompet — tanpa perlu memasangkan perangkat" |
| 26 | `getStarted.downloads.extension.steps[2]` | “Muat yang belum dikemas” | UI fit | Low | Chrome's Indonesian button reads *Muat yang belum dibuka* | “Muat yang belum dibuka” (Load unpacked) — see Open items |
| 27 | `chrome.footer.tagline`; `chrome.docs.ui.browse/hide/pagerLabel` (draft) | "Tanpa seed phrase."; "Jelajahi dokumen" | terminology | Low | the only *seed phrase* in a file that says *frasa pemulihan* everywhere else; *dokumen* means "a document", the nav says *Dokumentasi* | "Tanpa frasa pemulihan."; "Jelajahi / Sembunyikan dokumentasi", "Halaman dokumentasi" |

Docs (`content/docs/id/`):

| # | File | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 28 | introduction, networks-and-fees, faq | "12 jaringan EVM… Ethereum… World Chain" | mistranslation (stale) | High | 24 built-in networks (C-net-1) | 24, with the full table |
| 29 | install | "tidak ada yang perlu diunduh dan tidak ada toko aplikasi… Aplikasi seluler asli segera hadir"; Android 9+; no `dapps` anchor | mistranslation (stale) + technical | High | extension and desktop exist and download; phones are paid store apps not yet out (C-plat-1); anchor test failed | rewritten page with platform table, dApps section and `<span id="dapps">` |
| 30 | create-wallet | "Vela menurunkan alamat Anda dari kunci publik passkey"; one passkey only; no `what-is-public` anchor | mistranslation (stale) + technical | High | address comes from all keys (C-addr-1); keys 1–7 fixed at creation (C-keys-1); anchor test failed | rewritten, with the Callout and the public-record list |
| 31 | create-wallet, recovery | "Kunci publik dan nama pilihan Anda dikirim ke Indeks Passkey" | mistranslation (stale) | High | understates what becomes public forever (C-reg-1: credential ID, authenticator model, flags, labels) | full list under *Apa yang publik* |
| 32 | recovery, passkeys, signers | recovery = "sinkronisasi keychain" in iCloud Keychain / Google Password Manager; "hanya dipakai dengan wajah atau sidik jari Anda" | mistranslation (stale) | High | any one of up to seven keys signs in; phones and security keys are keys too (C-keys-2, C-auth-1); the account list isn't synced (C-sync-1) | rewritten from the reference |
| 33 | networks-and-fees, faq | "deposit kecil yang tidak dapat dikembalikan untuk mengaktifkan akun relay gas" | mistranslation (stale) | High | no per-network deposit or gas account exists (C-fee-2) | the treasury top-up is optional, non-refundable and doesn't pay your transaction |
| 34 | networks-and-fees | "Tidak ada pemilih kecepatan"; fee = "biaya jaringan ditambah biaya layanan relay" | mistranslation (stale) | High | there is a speed choice, default fast (C-fee-3); fee is 3 × reserved gas at the higher price (C-fee-1) | full fee section |
| 35 | send-and-receive, clear-signing | "Vela menulis ulangnya menjadi jumlah terbatas dan menolak mengirimkan persetujuan yang masih tanpa batas" | mistranslation (stale) | High | Vela doesn't rewrite; you reduce it; signed permits are not capped (C-approve-1) | Callout with the 2^200 / 2^152 rule and what it does not stop |
| 36 | faq, whitepaper | "dompetnya dan empat layanan pendukungnya… dengan lisensi MIT" | mistranslation (stale) | High | the public-key index has no licence file (C-lic-1) | MIT list + index without licence |
| 37 | whitepaper | "dihitung dari kunci publik passkey"; "kontrak pintar yang sudah diaudit" as the whole trust list | mistranslation (stale) | High | C-addr-1; the trust list now includes the signing app, the domain, services and relay | rewritten from the reference |
| 38 | bybit-attack | "menghapus primitif peningkatan yang diandalkan serangan itu" | mistranslation (stale) | High | the owner-signed `delegatecall` primitive still exists in every Safe, Vela's included | "Kami tidak punya peran admin yang bisa direbut" + what that does not remove; self-call gap |
| 39 | clear-signing-self-host | no status line | mistranslation (stale) | High | the page is built but not published and no app sends it requests (C-signpage-1) | **Status** paragraph from the reference |
| 40 | account-contract | "Kontrak Safe dan modul penanda tangan WebAuthn diaudit… Kode aplikasi Vela sendiri belum diaudit" | mistranslation (stale) | Medium | the unaudited scope is apps, backend services and the registry contract (C-audit-1); signer factory and fallback handler missing from the table (C-acct-1) | rewritten; table of five contracts |
| 41 | security-audits | old page: no Certora/Nethermind reviews, no known issues, no gaps list; *terpasang* for "deployed" | mistranslation (stale) + terminology | Medium | the page's purpose is the current audit and gap list | rewritten; *di-deploy* |
| 42 | why-vela | "menjadikan kunci pertama sebagai kunci keamanan fisik"; "supaya dompet Anda tidak pernah bergantung pada perusahaan kami tetap online"; Callout linked to `security-audits` | mistranslation (stale) | Medium | two security keys are needed; the domain limit (C-rp-1); the Callout's Safe link belongs to account-contract | from the reference |
| 43 | all docs | *swakelola*, *rantai*, *dipasang* (deploy), *menerjemahkan* (decode), *bersumber terbuka* next to catalog *open source* | terminology | Low | three words for one concept across catalog and docs | the table above |

New: `self-hosting.md` (all six anchors in their English sections).

## Open items

- **Chrome's "Load unpacked" label.** Written as *Muat yang belum dibuka* with the
  English label in parentheses (catalog step and three docs). I could not check a
  current Indonesian Chrome build; the parenthesis keeps the step usable either way.
- **"Self-hosting" in the wallet UI.** The site says *hosting sendiri* (its sidebar
  and footer term since 059); the wallet's Service Endpoints screen links to
  *Panduan self-hosting →*. Both are understood; aligning them is a product call,
  so the site term was kept.
- **Pengaturan vs Setelan.** The wallet corpus mostly says *Pengaturan* but the main
  navigation label is *Setelan*; the docs follow the majority (*Pengaturan*). The
  inconsistency is in the app, not in this locale's site files.

## Result
reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the en + zh revision of the same day (fee wording, configurable relay chain
directory, hero subtitle, facts #3 and #4) into `id.json` and four docs. Terms
unchanged from the table above: *relay*, *direktori chain*, *data chain*, *men-deploy*,
*kunci privat*, *mendekode*, *hosting sendiri*. A running relay is *deployment
vela-relay*, as Indonesian developer writing borrows it.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | "Kunci privat passkey Anda tidak pernah dikirim ke Vela." | – | first sentence unchanged; it already said "is done" (*dilakukan*) |
| `home.hero.facts[2]` | "Yang Anda lihat adalah yang Anda tandatangani: sebelum Anda menyetujui, Vela mendekode transaksi persis seperti yang akan ditandatangani." + new link | – | Indonesian has no settled native term for WYSIWYS; the form mirrors the fee callout "Yang Anda lihat adalah yang Anda bayar". *persis seperti yang akan ditandatangani* carries "exact transaction" (decoded from what is signed) |
| `home.hero.facts[3]` | 059 string restored; new link "Cara tetap memakai dompet Anda kalau Vela menghilang" | – | **059 string kept**: "Kalau Vela berhenti beroperasi, Anda tetap bisa mengakses dompet Anda." matches en and zh (即使 Vela 停止服务…) and reads naturally. Finding #3 above (High, "a promise the domain limit makes false") is superseded by the founder's 2026-09-22 ruling. Link echoes the whitepaper heading *Kalau Vela menghilang* |
| `home.tradeoffs.items[0].body` | middle paragraph rewritten (one fee, to the relay, Vela's by default, formula + `#fee` link, relay keeps the rest) | – | both hrefs verbatim and unprefixed; closing paragraph's dash became ", dan", as en |
| `home.faq.items[6].a` | relay code-change clause replaced by the list of services you can run | – | second paragraph untouched |
| `roadmap.upcoming[1].body` | relay clause dropped | – | |
| docs `networks-and-fees` | `<span id="fee">` under "Berapa biayanya"; "sepuluh kali lipat" paragraph replaced (bold dropped, as en); "**Siapa yang menerimanya.**" added | – | |
| docs `faq` | fee bullet (relay choice, `#fee` link); shutdown answer without the parenthesis | – | |
| docs `whitepaper` | intro clause, fee bullet + new bullet on who gets the fee, "Kalau Vela menghilang" clause | – | |
| docs `self-hosting` | intro limit removed; two code comments; "Perlu diketahui" bullet; chain-data paragraph; relay line removed from "Yang masih mengarah ke Vela" | – | |

No High or Medium findings open.
