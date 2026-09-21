# tr — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), acting as native Turkish localizer · Method: single-string locale review, whole pages

Scope: `messages/tr.json` (every key the brief lists, plus the chrome drafts and the register
defects found on the way) and all 17 docs in `content/docs/tr/`, with `self-hosting.md` new. Every
page was rewritten from the current English and Chinese. The old Turkish was translated from a much
older English, so it was used only for terminology and phrasing whose meaning still matched (chiefly
`why-vela`, `signers`, `bybit-attack` and the unchanged landing strings). Where a fact is involved
the claim ledger decided the wording, and qualifiers ("en fazla", "varsayılan olarak", "çoğu
zaman", "henüz", "herhangi biri") were checked in every place they appear.

## Terminology and register

| Concept | tr | Note |
| --- | --- | --- |
| key (any signing credential) | anahtar | umbrella term for a passkey, a phone or a security key |
| passkey | geçiş anahtarı | 059 choice, kept |
| security key | güvenlik anahtarı; hardware security key = donanım güvenlik anahtarı; USB güvenlik anahtarı | matches the app (`onboarding.create.methodSecurityKeyTitle` = "USB güvenlik anahtarı") |
| signer (a key as a Safe owner) | imzalayıcı; the people who sign at Bybit = imzacı | keeps a key and a person apart |
| relay | relay (relay'e, relay'in, relay'i) | kept as the loanword the old catalog and docs already used, and the app's field label "VELA RELAY". ERC-4337's role name given once as *bundler*; vela-relay's own hot wallets stay *relayer* as its docs name them |
| registry | kayıt defteri; registry contract = kayıt defteri sözleşmesi | the app's word for the ERC-7730 registry too |
| public-key index | açık anahtar dizini | "açık anahtar" is the app's term for public key; the old docs mixed "genel anahtar" and a capitalised "Geçiş Anahtarı Dizini" |
| self-hosting | kendi sunucunuzda barındırma (guide: Kendi sunucunuzda barındırma kılavuzu); verb: kendiniz barındırmak / çalıştırmak | replaces "Kendi kendine barındırma", which reads as "hosting by itself" |
| signing page | imza sayfası | |
| clear signing / blind signing | açık imzalama / kör imzalama | the sidebar's and the app's terms (`clearSigningTitle` = "Açık İmzalama Testi") |
| relying party | bağlı olan taraf (relying party) | Microsoft's Turkish term; English in parentheses at first use. The old "ilgili taraf" means "the party concerned" |
| trade-off | bedel (landing, why-vela); a weighed choice = denge | the old docs used "takas", which in a wallet means *swap* — the intent label the app shows (`intentSwap` = "Takas") |
| deploy / precompile | dağıtmak, dağıtım / ön derleme | as the old catalog |
| seed phrase | kurtarma ifadesi | 059 choice, kept (Base's "recovery phrase" takes the same word) |
| native coin / stablecoin | yerel coin / stabilcoin | |
| activity | geçmiş | the app's tab name (`history.navTitle` = "Geçmiş") |
| gas | gas, mostly in compounds (gas ücreti, gas bedeli, gas fiyatı); suffixed as gas'ı | the old text alternated "gas" and "gaz" |

UI paths are the app's Turkish labels: Ayarlar → Gelişmiş → Servis Uç Noktaları, Ayarlar → Ağlar,
speeds yavaş / standart / hızlı, buttons Al / Gönder / Cüzdan Oluştur / Varsayılanlara Sıfırla,
fields Zincir Veri Dizini / Vela Relay / Fiat Kurları, intents Gönder / Onayla / Takas. Chrome and
Windows labels are the official Turkish ones (Geliştirici modu, Paketlenmemiş öğe yükle, Ek bilgi,
Yine de çalıştır, Chrome Web Mağazası).

Register: formal *siz* throughout, as recorded in 059. Imperatives take -In (Başlayın, Giriş yapın,
Uzantıyı indirin). The one short dismiss button (*Kapat*) keeps the bare stem, as Turkish system
dialogs do. Suffixes on names follow pronunciation: Safe'in / Safe'inizde, GitHub'da, getvela.app'e,
`Safe{Wallet}`'inki, `delegatecall`'u.

## Findings

Before = the old Turkish text; After = what the file says now. Paths are relative to
`app-web/getvela.app/src/`.

### Catalog — `lib/i18n/messages/tr.json`

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | `home.meta.description`, `.ogDescription`, `.organization` | "…Geçiş anahtarıyla imzalayın — kurtarma ifadesi yok, donanım cüzdanı gerekmiyor, kimseye bağımlılık yok. Kolaylık için ödüyorsunuz…" | mistranslation | Medium | the search snippet says passkey only and "no hardware key", while the wallet signs with security keys; drops the unmodified Safe and the open-source services | "Bize ihtiyaç duymadan çalıştırabileceğiniz… Geçiş anahtarları ya da güvenlik anahtarlarıyla imzalayın… Hesabınız değiştirilmemiş bir Safe…" |
| C2 | `home.hero.facts[2].link` | "…ve biz o yolu nasıl kapatıyoruz" | mistranslation | Medium | claims the path is closed; the page it links to says mitigated, not eliminated | "…ve Vela buna karşı ne yapıyor" |
| C3 | `home.hero.facts[3].term` | "Vela hizmeti durdursa bile cüzdanınıza yine de erişebilirsiniz." | mistranslation | High | unqualified promise; access without getvela.app has the limits of C-rp-1 | "Vela'nın cüzdanınız için çalıştırdığı her şey açık kaynak ve hepsini kendiniz çalıştırabilirsiniz." |
| C4 | `home.hero.facts[3].link` | "Uygulamayı, relay'i ve tüm arka uç servislerini kendiniz çalıştırın" | mistranslation | Medium | hides that some things still depend on Vela | "Neyi çalıştırmanız gerekir, getvela.app olmadan neler çalışmaya devam eder, neler hâlâ bize bağlı" |
| C5 | `home.seal.label`, `.verify` | "zincir üstünde oluşturulmuş cüzdan" | mistranslation | Low | the counter counts registrations | "zincir üstünde kayıtlı cüzdan" / "…kayıt defterine bakın" |
| C6 | `home.why.p1` | `href="https://account.base.app"`; "…servis kapanırsa cüzdan da onunla birlikte gidiyor." | technical | High | href no longer the English one (FR-031 test failed); the closing claim is the one the English corrected | new href; "…kodu açık olmayan bir imzalama servisi — o servis ortadan kalkarsa geçiş anahtarınızın hesabınıza ulaşmasının yayımlanmış bir yolu yok." |
| C7 | `home.tradeoffs.items[0]` | "…ücret, zincir üstündeki maliyeti ve relay hizmet bedelini kapsar; tutar siz imzaladığınız anda sabitlenir"; link to the vela-relay repo | mistranslation / technical | High | understates the fee (no 3× reserve, no "often ten times or more", no minimum) against C-fee-1; href mismatch | full fee rule, "çoğu zaman gerçek zincir üstü maliyetin on katı ya da daha fazlası", `/docs/self-hosting#relay` |
| C8 | `home.tradeoffs.items[1].body` | "Cüzdanı oluştururken birden fazla anahtar tanımlayabilirsiniz" | mistranslation | Medium | drops "up to seven" and "can't be changed" (C-keys-1) | "…en fazla yedi anahtar seçersiniz ve bunlar sonradan değiştirilemez." |
| C9 | `home.tradeoffs.items[2]` | title "Denetlenmiş Safe sözleşmelerine dayanıyorsunuz — ve denetim bir garanti değildir"; body silent on Vela's own code | mistranslation | High | the ledger check for C-audit-1: this trade-off must say Vela's own code is unaudited and nothing is scheduled | "Sözleşmeler denetlendi; Vela'nın kendi kodu denetlenmedi" + "…üçüncü taraf denetiminden geçmedi ve takvime alınmış bir denetim de yok." |
| C10 | `home.compare.rows` | 12 rows | technical | High | English has 13 (new "Adding a key later"); `tr.home` failed the fragment test | 13 rows; row 7 "Sonradan anahtar eklemek" |
| C11 | `home.compare.rows[9].vela` ("An extra check") | "Bağımsız, kendi barındırdığınız bir sayfa ya da uzantı" | mistranslation | High | presents the signing page as available; C-signpage-1 says built, not published, not connected | "Bağımsız bir imza sayfası; hazır ama henüz uygulamalara bağlanmadı" |
| C12 | `home.compare.rows[10].vela` ("Full self-hosting") | "Uygulama, relay ve arka uç servisleri; hepsini siz çalıştırabilirsiniz" | mistranslation | High | omits the gaps and that passkeys stay tied to getvela.app (C-rp-1) | "…kendi sunucunuzda barındırma kılavuzunun listelediği eksiklerle; geçiş anahtarları getvela.app'e bağlı kalır" |
| C13 | `home.compare.rows[11]` ("Source code") | feature "Açık kaynak"; cell lists only what is open | mistranslation | Medium | the index has no licence file yet (C-lic-1); MetaMask/Base cells also stale | "Kaynak kod"; "…dizin dışında hepsi MIT, dizinin lisansı henüz eklenmedi" |
| C14 | `home.compare.rows[1]` ("Signing key") | "Geçiş anahtarı" (Vela and Base) | mistranslation | Medium | drops security keys and "up to seven" for Vela, and the recovery phrase for Base | "Geçiş anahtarları ya da güvenlik anahtarları, en fazla yedi" / "Geçiş anahtarı ya da kurtarma ifadesi" |
| C15 | `home.compare.rows[3].metamask`, `.base` ("Sponsored gas") | "Sunulmuyor" / "Desteklenen yerlerde sponsorlu" | mistranslation | Medium | MetaMask sponsors on some networks; Base only where the app sponsors | "Bazı ağlarda" / "Uygulama sponsor olduğunda" |
| C16 | `home.compare.rows[0]`, `[2]`, `[4]`, `[7]` | "EOA"; "…sözleşmeleri kendiniz de dağıtabilirsiniz"; "Daha dar bir ağ yelpazesi" | mistranslation | Low | English refined (EIP-7702 default, "most missing contracts", "fixed list") | aligned |
| C17 | `home.pricing.cards` | "Masaüstü ve mobil uygulamalar — Kendiniz derlerseniz ücretsiz"; "Uygulama mağazalarından — Tek seferlik satın alma" | mistranslation | High | desktop is a free download, not source-only; store apps are not available yet (C-plat-1) | "Web, tarayıcı uzantısı ve masaüstü — Ücretsiz" / "…kendiniz derlerseniz — Ücretsiz" / "…mağazalardan — Tek seferlik satın alma · yakında" |
| C18 | `home.networks.heading`, `.body` | "12 ağ hazır gelir…"; link to biubiu.tools | mistranslation / technical | High | 24 networks (C-net-1; network-count test failed); href mismatch | "24 ağ hazır geliyor…"; `/chain-setup` "zincir kurulumu" |
| C19 | `home.faq.items[0].a` | "Face ID veya parmak iziyle açılan bir cihaz ya da bir USB/NFC güvenlik anahtarı…" | mistranslation | Medium | face/fingerprint as the only way (C-auth-1); omits "two if only security keys" and "up to seven" | "Geçiş anahtarlarını destekleyen bir telefon ya da bilgisayar veya donanım güvenlik anahtarları — yalnızca güvenlik anahtarı kullanacaksanız iki tane…" |
| C20 | `home.faq.items[1].a` | "…iOS, Android ve masaüstü uygulamalarında ve Vela tarayıcı uzantısında çalışır." | mistranslation | Medium | implies Linux desktop; omits that the web wallet doesn't connect (C-dapp-1) | "…masaüstü (macOS, Windows), iPhone ve Android… Web cüzdanı dApp'lere bağlanmaz." |
| C21 | `home.faq.items[3].a` | "Tek anahtarınız oysa cüzdana erişiminizi de kaybedersiniz." | mistranslation | Medium | "oysa" (whereas) for "idiyse" (if it was): the condition is garbled. Not in the brief's list; a real defect | "Tek anahtarınız o idiyse cüzdana erişiminizi de kaybedersiniz." |
| C22 | `home.faq.items[4].a` | "Eşitlenmiş geçiş anahtarlarınıza ulaşan biri, cüzdanınıza da ulaşabilir…" | mistranslation | High | missing the action that protects the money: a key can't be removed, so move funds to a new wallet | "…ve bir anahtar kaldırılamaz. Paranızı, o hesaba bağlı olmayan anahtarlarla oluşturulmuş yeni bir cüzdana taşıyın." |
| C23 | `home.faq.items[5].a` | "Vela paranızı ne taşıyabilir ne de dondurabilir… Açık anahtarınız, cüzdan adresiniz ve cüzdana verdiğiniz ad…" | mistranslation | Medium | omits that Vela writes the signing software, the authenticator kind and key labels (C-reg-1), and what the services see | full two-paragraph answer |
| C24 | `home.faq.items[6].a` | "…getvela.app erişilemez olursa tarayıcıdan imzalayın: Vela uzantısı ya da bağımlılıksız açık imzalama uzantısı…" | mistranslation | High | offers the unpublished signing page as a working route (C-signpage-1); omits the relay code change | "…Vela tarayıcı uzantısı mevcut anahtarlarınızla imzalamayı sürdürür…; kendiniz derlediğiniz uygulamalar da…" |
| C25 | `about.meta.description`, `about.lede` | "…Arkada yüzü olmayan bir şirket yok…"; "…cüzdanı, akıllı sözleşmeleri…" | mistranslation | Medium | the page now names MONDAY LABS LTD; "the smart contracts" is not what Vela builds in the funds path | "Vela'yı Birleşik Krallık'taki küçük bir şirket olan MONDAY LABS LTD yapıyor…" |
| C26 | `about.values[0].body` | "Anahtarlar sizin, coin'ler sizin — bir slogan değil, mimarinin kendisi…" | mistranslation | Medium | drops "what we do control is the software you sign with" | "Kontrol ettiğimiz şey, imzalarken kullandığınız yazılım; kodunun açık olmasının nedeni de bu." |
| C27 | `about.values[1].body` | "…Onun yerine geçiş anahtarı koyduk: yüzünüz ya da parmak iziniz." | mistranslation | Medium | a passkey is not your face (C-auth-1); security keys and PIN missing | "…cihazlarınızın ya da bir güvenlik anahtarının tuttuğu, yüzünüz, parmak iziniz ya da PIN'inizle onayladıktan sonra kullanılan anahtarlar." |
| C28 | `about.values[2].body`, `about.team.bio` | "Cüzdan GitHub'da herkese açık…"; "…cüzdanı, sözleşmeleri…" | mistranslation | Low | apps and services, not "the wallet" / "the contracts" | aligned |
| C29 | `roadmap.upcoming`, `roadmap.shipped` | old 5 + 7 items, incl. "Tüm cihazlarınız arasında eşitleme… hesaplarınız zaten platformun yedeğiyle birlikte geliyor" and "Daha uzağa: …P-256 ön derlemesi olmayan zincirler için bir imzalama yolu… bağımsız bir güvenlik denetimi" | mistranslation / technical | High | contradicts C-sync-1 and C-p256-1, and lists an audit as coming (forbidden); `tr.roadmap` failed the fragment test | the new 5 + 10 items, replaced entirely |
| C30 | `getStarted.meta.description`, `.lede` | "…masaüstü, mobil ve tarayıcı uzantısı aynı koddan derleniyor"; "…masaüstünüze, telefonunuza…" | mistranslation | Medium | implies the phone apps are available (C-plat-1) | "…telefon uygulamaları da yolda"; "…yakında telefonunuza da." |
| C31 | `getStarted.platforms.web.blurb` | "…geçiş anahtarınızla kimliğinizi doğrulayın…" | mistranslation | Medium | any one of your keys; omits "use the extension for dApps" | "Anahtarlarınızdan biriyle onaylayın… dApp'lere bağlanmak için uzantıyı kullanın." |
| C32 | `getStarted.platforms.desktop.stores` | "Mac App Store · Microsoft Store" | mistranslation | Medium | Mac App Store is no longer a channel | "Microsoft Store" |
| C33 | `getStarted.fundingNote` | "…kendiniz de derleyebilirsiniz — aynı uygulama, ücretsiz." | mistranslation | Medium | a self-built phone app can't use the phone's own passkey (C-rp-1); "same app" is false | "…kendi derlediğiniz bir telefon uygulaması, telefonun kendi geçiş anahtarıyla değil, başka bir telefonla ya da bir güvenlik anahtarıyla imzalar." |
| C34 | `chrome.docs.titles["clear-signing-self-host"]` and the doc's title/H1 | "İmza sayfasını kendin barındır" | cultural risk | Medium | informal *sen* in a formal-*siz* locale, in the sidebar every docs page shows | "İmza sayfasını kendiniz barındırın" |
| C35 | `chrome.footer.links.selfHosting`, `chrome.docs.titles["self-hosting"]` | "Kendi kendine barındırma kılavuzu" | unnatural | Medium | "kendi kendine" means "by itself / on its own" | "Kendi sunucunuzda barındırma kılavuzu" |
| C36 | `chrome.docs.ui.hide` | "Belgeleri gizle" (beside "Belgelere göz atın") | cultural risk | Low | the two states of one toggle in two registers | "Belgeleri gizleyin"; `edit` apostrophe normalised to GitHub'da |
| C37 | `chrome.nav.signIn`, `notice.offer.accept`, `getStarted.downloads.extension.action` | "Giriş yap", "Türkçe oku", "Uzantıyı indir" | cultural risk | Low | informal next to the site's formal CTAs ("Vela'yı edinin", "Web cüzdanını açın") | "Giriş yapın", "Türkçe okuyun", "Uzantıyı indirin" |
| C38 | `getStarted.platforms.extension.stores` | "Chrome Web Store" | UI fit | Low | the same page's steps say "Chrome Web Mağazası", the store's Turkish name | "Chrome Web Mağazası" |
| C39 | `home.compare.heading` | "Vela nasıl karşılaştırılır" | unnatural | Low | calque; reads "how is Vela compared / how to compare Vela" | "Vela ve diğer cüzdanlar" |
| C40 | `roadmap.meta.description`, `roadmap.lede`, `about`, `getStarted.fundingNote` | "açıkta geliştiriliyor" | unnatural | Low | calque of "in the open" (reads "out in the open air") | "herkese açık biçimde geliştiriliyor" |

### Docs — `content/docs/tr/`

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| D1 | `introduction.md` | "**12 ağ, tek adres.** Ethereum, BNB Chain… World Chain" | mistranslation | High | 24 networks (C-net-1) | "**24 ağ, tek adres.**…" and the new "Bir yanıt bulun" table |
| D2 | `introduction.md` | "…yüzünüzle ya da parmak izinizle, bir geçiş anahtarıyla imzalıyorsunuz"; "İmza anahtarınız, cihazınızın güvenli donanımında duran bir geçiş anahtarı" | mistranslation | Medium | one kind of key, one kind of check (C-keys-2, C-auth-1) | "…telefonunuzda ya da bilgisayarınızda, başka bir telefonda ya da bir donanım güvenlik anahtarında"; "en fazla yedi" |
| D3 | `install.md` | "Vela tarayıcınızda çalışır — indirilecek bir şey ve geçilmesi gereken bir uygulama mağazası yok… Yerel mobil uygulamalar yakında" | mistranslation | High | C-plat-1 forbids "nothing to download / no app store"; the extension and desktop apps were missing | platform table, extension, desktop and phone sections |
| D4 | `install.md` | no `<span id="dapps">` | technical | High | anchor linked from elsewhere; test failed | "Vela'yı dApp'lerle kullanma" with the anchor |
| D5 | `create-wallet.md` | "Vela adresinizi geçiş anahtarınızın genel anahtarından türetir" | mistranslation | High | the address comes from all founding keys (C-addr-1) | "Uygulama cüzdanınızın adresini anahtarların tamamından hesaplar…" |
| D6 | `create-wallet.md` | no `<span id="what-is-public">` | technical | High | anchor linked from intro, recovery and FAQ; test failed | "Neler herkese açık" with the anchor |
| D7 | `create-wallet.md` | "Genel anahtar ve seçtiğiniz ad, Vela'nın Geçiş Anahtarı Dizini'ne gönderiliyor…" | mistranslation | Medium | the public record is larger (credential ID, authenticator model, flags, key labels, signed data — C-reg-1) | full five-item list |
| D8 | `why-vela.md` | Callout link `/tr/docs/security-audits` on "Safe akıllı hesabı" | technical | Medium | the English links the account contract | `/tr/docs/account-contract` |
| D9 | `why-vela.md` | "…en baştaki ilk anahtarı bir donanım güvenlik anahtarı yapabilirsiniz" | mistranslation | Medium | a wallet whose only key syncs nowhere can't be created; the English says hardware keys only — two of them | "…yalnızca donanım güvenlik anahtarları kullanabilirsiniz — iki tane…" |
| D10 | `why-vela.md` | "…cüzdanınız şirketimizin ayakta kalmasına hiç bağlı olmasın diye…" | mistranslation | Medium | overclaim; the domain limit (C-rp-1) now stated with a link | "…mevcut bir cüzdan şirketimizin sunucuları olmadan da çalışmaya devam etsin diye… tek bir sınırla: … alan adı" |
| D11 | `why-vela.md` (and old `recovery`, `whitepaper`, `signers`) | "takas" for trade-off | terminology | Medium | collides with the Swap intent ("Takas") on the signing screen | "bedel" / "denge" |
| D12 | `send-and-receive.md` | "Sınırsız token onayı… Vela onu sonlu bir tutara yeniden yazar ve hâlâ sınırsız kalacak bir onayı göndermeyi reddeder." | mistranslation | Medium | no permit caveat (C-approve-1); now handled on the clear-signing page | section replaced by the English's fee, names, split/sweep sections |
| D13 | `networks-and-fees.md` | "Vela, içinde **12 EVM ağıyla** geliyor" | mistranslation | High | 24 (C-net-1) | 24-network table |
| D14 | `networks-and-fees.md` | "Hız seçici yoktur: her işlem yüksek öncelikle gönderilir." | mistranslation | High | contradicts C-fee-3 | "…bir hız seçebilirsiniz (varsayılan: hızlı)" |
| D15 | `networks-and-fees.md` | "Gaz hesabını etkinleştirmek… iade edilmez… depozito" | mistranslation | High | the gas account no longer exists (C-fee-2) | "Cüzdan başına bir gas hesabı ya da etkinleştirme depozitosu yoktur…" + treasury section |
| D16 | `networks-and-fees.md` | "Toplam tutar, ağ maliyeti artı relay'in hizmet ücretidir…" | mistranslation | High | the fee is 3 × reserved gas at the higher price, often ≥10× the real cost (C-fee-1) | "Ücret = 3 × ayrılan gas × gas fiyatı…" |
| D17 | `passkeys.md` | "…yalnızca yüzünüzle ya da parmak izinizle kullanılır"; "Özel anahtar işletim sisteminizin geçiş anahtarı sağlayıcısında durur" | mistranslation | Medium | security keys and PIN missing (C-keys-2, C-auth-1) | three-row key-kind table; "Face ID, parmak izi, cihaz PIN'iniz ya da güvenlik anahtarına dokunup PIN girerek" |
| D18 | `passkeys.md` | "4. Vela imzalı işlemi ağa yayınlar." | mistranslation | Medium | the relay submits it | "Uygulama imzalı işlemi relay'e verir, relay de onu zincire gönderir…" |
| D19 | `signers.md` | "Sonradan sekizinci bir imzalayıcı eklemek…" | mistranslation | Medium | English: "another signer"; "eighth" implies you already have seven | "Sonradan başka bir imzalayıcı eklemek…" |
| D20 | `signers.md` | "…oluştururken bir YubiKey kaydedin ve onunla imzalayın." | mistranslation | Medium | a single unsynced key can't be created; two security keys | "…cüzdanı **iki** güvenlik anahtarıyla oluşturun…" + app-support table and "Bir anahtar ele geçirilmiş olabilirse" |
| D21 | `recovery.md` | "Anahtar zinciri eşitlemesi açık olacak şekilde aynı iCloud ya da Google hesabına girin…"; Callout "Kurtarmanız platform anahtar zincirinize bağlı" | mistranslation | High | sign-in works with any one key, not only the synced platform key (C-sync-1, C-keys-1) | "Anahtarlarınızdan **herhangi birini** kullanın…" |
| D22 | `recovery.md` | "…Vela, genel anahtarınızı iki geçiş anahtarı imzasından cihaz üzerinde yeniden kurabilir…" (for every wallet) | mistranslation | High | only a single-key wallet can be rebuilt without the registry | Callout "Tek anahtarlı bir cüzdan…; birden fazla anahtarı olan bir cüzdan kayıt defteri kaydına ihtiyaç duyar" |
| D23 | `clear-signing.md` | Callout "Sınırsız onaylar engellenir" — "…isteği sizin seçtiğiniz sonlu bir tutara yeniden yazar…" | mistranslation | High | the ledger check for C-approve-1: no "unlimited approvals are blocked" without the permit / large-finite / setApprovalForAll caveat | "Zincir üstündeki 'sınırsız' onaylar gönderilemez" + "Durdurmadığı şeyler: …" |
| D24 | `clear-signing.md` | "Vela asla öylece 18 varsaymaz…" | mistranslation | Medium | it does display as if 18, marked unverified | "…token 18 ondalıklıymış gibi gösterir ve doğrulanmamış olarak işaretler" |
| D25 | `clear-signing.md` | "doğrulanmış" with no qualification | mistranslation | Medium | fetched descriptors are not cryptographically authenticated (C-clear-1) | "'Doğrulanmış', bu sözleşme için bir tanımlayıcı bulundu demektir; kriptografik olarak kontrol edildiği anlamına gelmez…" |
| D26 | `bybit-attack.md` | "Yükseltebileceğimiz bir sözleşme yok…"; "…saldırının dayandığı yükseltme yeteneğini ortadan kaldırmak" | mistranslation | High | the owner-signed `delegatecall` primitive still exists in every Safe, Vela's included | "Kaybedebileceğimiz bir yönetici rolü yok… Bunun neyi ortadan *kaldırmadığı* konusunda net olalım…" |
| D27 | `bybit-attack.md` | (absent) | mistranslation | High | the self-call gap (`enableModule`, `addOwnerWithThreshold`…) and "reject any request whose target is your own address" were missing | two-limits paragraph |
| D28 | `bybit-attack.md` | "Vela… imza sayfası geliştiriyor… Yayına girdiğinde tercihe bağlı olacak" | mistranslation | Medium | status must say not published and no app sends requests (C-signpage-1) | "*Durum: hazır ve test edildi; yayımlanmadı ve henüz hiçbir Vela uygulaması ona istek göndermiyor.*" |
| D29 | `account-contract.md` | "…Safe uyumlu her arayüz onu sürebilir." | mistranslation | High | any Safe tool can read and build; signing needs a getvela.app signature (C-safeui-1) | "…Safe'in kendi web uygulaması sizin adınıza imzalayamaz." |
| D30 | `account-contract.md` | "Zincire göre, düz bir EOA transferinin kabaca 1,5–3 katı gaz bekleyin." | mistranslation | High | the measured figure is ~140,000–170,000 gas vs 21,000 | "…kabaca 140.000–170.000 gas… 21.000…" |
| D31 | `account-contract.md` | "Sizinle paranız arasında dört sözleşme duruyor" (links to `main`) | mistranslation | Medium | the signer factory and its signers were missing; links point at unpinned branches | five-row table with pinned links |
| D32 | `account-contract.md` | "**Vela'nın kendi uygulama kodu** bağımsız bir denetimden geçmedi" | mistranslation | Medium | C-audit-1 scope is apps, backend services and the registry contract | "Vela'nın kendi kodu — uygulamalar, arka uç servisleri ve kayıt defteri sözleşmesi —…" |
| D33 | `security-audits.md` | "Tempo ve pathUSD. On iki yerleşik ağımızdan biri…" | mistranslation | High | 24 (C-net-1) | "24 yerleşik ağdan biri" |
| D34 | `security-audits.md` | verify table: "Geçiş anahtarı genel anahtar dizini (Gnosis) `0xdd93…E9c3`"; repo link to the old index | mistranslation | High | the current registry is `0x94fD…1EA9`; `0xdd93…` is the read-only predecessor | table row "Açık anahtar kayıt defteri (Vela, denetlenmedi)"; old address described as history |
| D35 | `security-audits.md` | "…en kötü durum, zaten kabul ettiğiniz ücretle sınırlı" | mistranslation | Medium | with the in-band fee the relay absorbs the gas; mempool exposure narrowed, not removed | "…gas bedelini siz değil relay üstlenir"; "…maruziyeti ortadan kaldırmaz, daraltır" |
| D36 | `security-audits.md` | (absent) Certora M-01, Nethermind reviews, signer factory, "Vela'nın kendi savunmalarındaki eksikler" | mistranslation | Medium | known issues and gaps the English lists | added |
| D37 | `faq.md` | "Vela **12 EVM ağıyla** geliyor" | mistranslation | High | 24 (C-net-1) | "24 yerleşik EVM ağı" |
| D38 | `faq.md` | "Her ağ ayrıca gaz relay hesabını etkinleştirmek için küçük ve iade edilmeyen bir depozito ister" | mistranslation | High | no deposit, no gas account (C-fee-2) | "Depozito ya da abonelik yoktur." |
| D39 | `faq.md` | "…cüzdan ve dört arka uç servisi… MIT lisansıyla" | mistranslation | High | the index has no licence file (C-lic-1) | "…Açık anahtar dizini herkese açık ama henüz bir lisans dosyası yok." |
| D40 | `faq.md` | lost phone: "…yeni bir cihazda aynı hesapla giriş yaparsınız" | mistranslation | Medium | any other key works; an unsynced only key is unrecoverable (C-sync-1) | "Yeni bir cihazda başka herhangi bir anahtarla giriş yapın…" |
| D41 | `whitepaper.md` | "Vela 12 EVM ağını destekliyor…" | mistranslation | High | 24 (C-net-1) | 24-network list |
| D42 | `whitepaper.md` | "…gaz hesabınızı fonlu tutan relay… Gizli marj yok… iade edilmeyen bir depozito" | mistranslation | High | C-fee-1 / C-fee-2 | "Ücretler" section: 3× reserve, higher price, no deposit |
| D43 | `whitepaper.md` | "…geçiş anahtarınızın genel anahtarından `CREATE2` ile hesaplanır" | mistranslation | High | all founding keys (C-addr-1) | "…kurucu anahtarların hepsini içeren Safe kurulum verisinden…" |
| D44 | `whitepaper.md` | "…alan adı kaybı için tüketici düzeyinde bir kurtarma yolu hâlâ bitmemiş bir iş…" (extension "removed") | mistranslation | High | the extension and self-built apps keep signing without getvela.app (C-rp-1) | "Vela ortadan kalkarsa" rewritten |
| D45 | `whitepaper.md` | "Her şey **MIT lisanslı**… dört arka uç servisinin tamamı" | mistranslation | High | C-lic-1 | "…Açık anahtar dizini herkese açıktır ama henüz bir lisans dosyası yoktur." |
| D46 | `whitepaper.md` | "Güvenmeniz gereken şey… sözleşmelere, geçiş anahtarı kasasına ve… relay'e iner." | mistranslation | High | the threat model now includes the app's code and the domain; "only for liveness" understated the relay | "Neye güveniyorsunuz" list: sözleşmeler, alan adı, kimlik doğrulayıcılar, uygulamanın kodu, RPC, servisler, relay |
| D47 | `whitepaper.md` | "Vela'nın onların etrafındaki kendi entegrasyonu… denetiminden geçmedi" | mistranslation | Medium | C-audit-1 scope | "Vela'nın kendi kodu — uygulamalar, arka uç servisleri ve kayıt defteri sözleşmesi —…" |
| D48 | `clear-signing-self-host.md` | no status; "Ne zaman kullanmalı: Hesap… parayı tuttuğu gün — ve o günden sonra her imza için." | mistranslation | High | tells the reader to use a page no app can hand requests to (C-signpage-1) | "**Durum:** … **yayımlanmadı** ve **henüz hiçbir Vela uygulaması ona istek göndermiyor**"; "Nereye oturuyor" in the future tense |
| D49 | `clear-signing-self-host.md` | "…kendi başına hiçbir ağ isteği yok"; "Ağ isteği yok." | mistranslation | Medium | it loads decorative token logos from Vela's chain-data server | "…yaptığı tek istek, süs amaçlı token logoları için" |
| D50 | `clear-signing-self-host.md` | "ilgili tarafı" for relying party | terminology | Medium | "the party concerned"; the WebAuthn term is bağlı olan taraf | "bağlı olan tarafı (relying party)" |

`self-hosting.md` did not exist in Turkish; it was written new from the English and Chinese, with
all six anchors in their sections.

Counts, old text: **High 40** (catalog 12, docs 28), **Medium 42** (catalog 20, docs 22), all fixed;
Low findings listed above are fixed too.

## Open items

- `relay` is kept as a loanword. A native "aktarıcı" exists and the app uses it once, in
  parentheses (`componentsUi` gas-top-up disclaimer); switching would touch every page and the app,
  so it is left to a product-wide terminology decision.
- The app's own Turkish says "passkey" in several labels ("PASSKEY DİZİNİ", "passkey'i") and
  "öz-saklama" for self-custody, where the site says "geçiş anahtarı" and "kendi saklamanızda". The
  site follows the 059 terminology; aligning the app is outside this scope.
- The English title of `home.tradeoffs.items[2]` carries full stops again, against the 059 rule that
  trade-off titles take none; `zh` follows the rule, and so does `tr` ("…denetlendi; …denetlenmedi").

## Result

reviewed — no open High or Medium findings
