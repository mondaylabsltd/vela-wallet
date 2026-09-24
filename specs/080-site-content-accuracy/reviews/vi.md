# vi — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), vi localizer · Method: single-string locale review, whole pages

Scope: every key the brief lists in `vi.json`, plus the defects found in keys it did
not list; all 17 docs rewritten from the current English and Chinese
(`self-hosting.md` created). The old vi docs were translated from an English that
predates spec 080 (12 networks, a per-network gas account with an activation
deposit, an address derived from one passkey synced by iCloud or Google, "nothing to
download", passkey = "your face or fingerprint"), so most findings below are
accuracy: a Vietnamese reader was being told things the product no longer does, or
never did. Where the old Vietnamese was still right and read well (much of
`why-vela` and `bybit-attack`), its phrasing was kept.

## Terminology and register

| Concept | vi | Note |
| --- | --- | --- |
| key (any signing credential) | khóa | umbrella term, as the file and the wallet UI (`Khóa`, `Thêm một khoá nữa`) use it; spelled *khóa* throughout the site |
| signer (a Safe owner) | khóa ký; owners = chủ sở hữu | as the sidebar title *Khóa ký & khóa bảo mật* |
| passkey | passkey | 059 choice, kept (see Open items) |
| security key / hardware security key | khóa bảo mật / khóa bảo mật phần cứng | the wallet onboarding's own terms (*Khoá bảo mật USB*, *Khóa bảo mật*); this is also what Google's Vietnamese UI calls a FIDO key |
| relay | relay | kept in English, as the product (`vela-relay`, UI field *VELA RELAY*) does; **unified** — the old catalog mixed *relay* and *relayer*; *relayer* now only for the relay's own sender addresses in the self-hosting guide |
| registry | sổ đăng ký / hợp đồng sổ đăng ký | the wallet UI's term (*Không kết nối được sổ đăng ký*); *hợp đồng sổ đăng ký* where the contract itself is meant |
| public-key index | chỉ mục khóa công khai | the service; the Settings field is quoted by its UI label, *chỉ mục passkey* |
| self-hosting | tự triển khai (guide: *Hướng dẫn tự triển khai*); run a service = tự chạy / tự vận hành | matches the wallet's *Hướng dẫn tự triển khai →* link and its endpoints note (*tự triển khai phiên bản của riêng mình*); the sidebar group "Run it yourself" = *Tự vận hành* |
| signing page | trang ký | 059 choice, kept |
| clear signing / blind signing | ký minh bạch / ký mù | 059 choice, kept; *ký mù* is also the wallet UI's term |
| self-custodial | tự lưu ký | **changed** from *tự quản* ("self-managed"), which is not the Vietnamese crypto term; *tự lưu ký* is what Vietnamese crypto media and the wallet's own endpoints note say |
| seed phrase / recovery phrase | cụm từ khôi phục | one term for both, as the file and the wallet UI already do; Base Account's browser-generated secret in `why-vela` stays *khóa khôi phục* (the English says "recovery key" there) |
| custom network | mạng tùy chỉnh | **changed** from *mạng tùy chọn* ("optional network"); the wallet UI says *Thêm mạng EVM tùy chỉnh* |
| native coin / fee coin | coin gốc / token trả phí | *token trả phí* is the confirm screen's label (*Token trả phí*) |
| relying party | bên phụ thuộc (relying party) | English term in parentheses at first mention on each page; **changed** from *bên tin cậy*, which reads as "trusted party" |
| authenticator / credential ID | trình xác thực / ID thông tin xác thực | the wallet's key-details labels (*Thông tin xác thực*) |
| treasury / operation | ngân quỹ / thao tác | *Ngân quỹ* is the wallet UI's term |
| build yourself | tự biên dịch | **unified** — the old catalog mixed *tự build*, *tự biên dịch*; *build* kept only inside code-level sentences ("bước build", "image") |
| iCloud Keychain / Google Password Manager | Chuỗi khóa iCloud / Trình quản lý mật khẩu của Google | Apple's and Google's Vietnamese names; **changed** in `home.faq.items[2].a`, which used the English names |
| Settings paths | Cài đặt → Mạng lưới; Cài đặt → Nâng cao → Điểm cuối dịch vụ; **Khôi phục mặc định** | the vi wallet corpus labels (`settings.advanced.networksTitle`, `settings.advanced.endpointsTitle`, `settingsModals.endpoints.resetToDefaults`); field names quoted as the app shows them: *chỉ mục dữ liệu chain*, *chỉ mục passkey*, *Vela Relay*, *tỷ giá fiat* |
| UI buttons | Tạo ví, Nhận, Gửi, Thiết bị này, Điện thoại hoặc máy tính bảng, Khoá bảo mật USB, Chậm / Tiêu chuẩn / Nhanh, Tối đa | as the wallet shows them |

Register: neutral *bạn* for the reader and *chúng tôi* for the team, as recorded in
059; imperatives softened with *hãy* where the English gives an instruction. A plain
product-documentation voice: no slang (*đụng tường*, *dí vào mặt* kept out of new
text), no hype. Vietnamese formatting: 0,01 USD, 1,5 tỷ USD, 140.000–170.000 gas,
1,1 triệu gas, dates as *21/2/2025* and *22/9/2026*. Chrome and Windows labels as
their Vietnamese builds show them (*Chế độ dành cho nhà phát triển*, *Tải tiện ích
đã giải nén*, *Windows đã bảo vệ PC của bạn*, *Thông tin thêm*, *Vẫn chạy*). Contract
names, EIP/ERC numbers, commands, file paths and addresses stay as in English;
comments inside code blocks and the prose of the whitepaper's text diagram are
translated, as `zh` does.

## Findings

| # | File / key | Before (old translation) | Type | Severity | Why | After |
|---|---|---|---|---|---|---|
| F-1 | `home.meta.description` | «…tự vận hành được… Ký bằng passkey — không cụm từ khôi phục, **không cần ví cứng**, không bị trói vào ai.» | mistranslation | High | contradicts C-keys-2 (hardware security keys are a first-class key kind) and states self-hostability without the domain limit (C-rp-1) | «Ví Ethereum mã nguồn mở mà bạn có thể tự vận hành, không cần đến chúng tôi. Ký bằng passkey hoặc khóa bảo mật…» |
| F-2 | `home.meta.ogDescription` | «Ví mã nguồn mở, tự vận hành được cho ETH và ERC-20… Muốn thì bạn tự biên dịch lấy.» | mistranslation | Medium | old claim set; drops the unmodified Safe and security keys | «Ví Ethereum mã nguồn mở dựng trên một Safe nguyên bản. Ký bằng passkey hoặc khóa bảo mật…» |
| F-3 | `home.meta.organization` | «…Ký bằng passkey — không cụm từ khôi phục, **không cần ví cứng**…» | mistranslation | High | same as F-1, in the structured data search engines quote | «Ví Ethereum mã nguồn mở, tự lưu ký, dựng trên một Safe nguyên bản, ký bằng passkey hoặc khóa bảo mật…» |
| F-4 | `home.hero.facts[2].link` | «…và lối đó chúng tôi chặn ra sao» | mistranslation | Medium | "how we block that path" overclaims; the reference says what Vela does about it (mitigated, not eliminated) | «…và Vela làm gì trước rủi ro đó» |
| F-5 | `home.hero.facts[3].term` / `.link` | «Dù Vela ngừng hoạt động, bạn vẫn truy cập được ví của mình.» / «Tự vận hành ứng dụng, relay và mọi dịch vụ phía sau» | mistranslation | High | unconditional promise contradicted by C-rp-1 (passkeys stay bound to getvela.app) and C-selfhost-2 | «Mọi thứ Vela vận hành cho ví của bạn đều là mã nguồn mở, và bạn có thể tự vận hành chúng.» / «Cần chạy những gì, thứ gì vẫn hoạt động khi không có getvela.app, và thứ gì vẫn phụ thuộc vào chúng tôi» |
| F-6 | `home.seal.label` / `.verify` | «ví đã được tạo on-chain» / «Mọi chiếc ví đều nằm trên chuỗi» | mistranslation | Medium | the counter counts registry records, not wallets deployed on-chain (a wallet deploys on its first send) | «ví đã đăng ký trên chuỗi» / «Mọi ví đều được đăng ký trên chuỗi — xem sổ đăng ký» |
| F-7 | `home.why.p1` | href `account.base.app`; «một khóa khôi phục được tạo ra ngay trong trình duyệt… nếu dịch vụ đóng cửa thì chiếc ví đi theo» | technical + mistranslation | High | href no longer matched English (the markup test failed); missing the closed signing service and "a recovery phrase created on a website" | new href; «một cụm từ khôi phục được tạo trên một trang web… và một dịch vụ ký có mã nguồn không công khai…» |
| F-8 | `home.tradeoffs.items[0]` | «Mặc định, relayer trả gas… Phí gồm chi phí trên chuỗi và phí dịch vụ relay… Bạn có thể đổi relayer trong phần cài đặt» | mistranslation | High | understates what the reader pays: contradicts C-fee-1 (3× reserved gas, often ≥10× the real cost) and C-relay-1; href changed to `/docs/self-hosting#relay` | rewritten to the formula, the 0,01 USD minimum, "không thể thay đổi sau đó", «trỏ ví sang một relay khác hoặc tự chạy relay của mình» |
| F-9 | `home.tradeoffs.items[1].body` | «Bạn có thể đặt nhiều khóa ngay khi tạo ví» (no full stop) | mistranslation | High | drops "up to seven" and "can't be changed afterwards" (C-keys-1) | «Bạn chọn tối đa bảy khóa khi tạo ví, và sau đó không thể thay đổi.» |
| F-10 | `home.tradeoffs.items[2]` | «Bạn đang dựa vào hợp đồng Safe đã được kiểm toán — và kiểm toán không phải là bảo đảm» | mistranslation | High | the reference's point — Vela's own code is not audited and none is scheduled (C-audit-1) — was absent | «Các hợp đồng đã được kiểm toán. Mã của chính Vela thì chưa.» + «…cũng chưa có lịch kiểm toán nào» |
| F-11 | `home.compare.rows` | 12 rows | technical | High | English has 13 (new "Adding a key later"); the row set no longer matched | 13 rows, «Thêm khóa về sau» in place |
| F-12 | `compare` "Một lớp kiểm tra nữa" / "Tự vận hành trọn vẹn" | «Một trang hoặc tiện ích độc lập, do bạn tự vận hành» / «Ứng dụng, relay và các dịch vụ phía sau, bạn chạy được tất cả» | mistranslation | High | presents the unpublished signing page as available (C-signpage-1) and full self-hosting without the domain gap (C-rp-1) | «Một trang ký độc lập, đã làm xong nhưng chưa kết nối với các ứng dụng» / «…còn vài thiếu sót được liệt kê trong hướng dẫn tự triển khai; passkey vẫn gắn với getvela.app» |
| F-13 | `compare` signing key, gas, sponsored gas, networks, source code | «Passkey»; MetaMask sponsored gas «Không có»; Base networks «Phạm vi mạng hẹp hơn»; «Mã nguồn mở: Ứng dụng, relay và các dịch vụ phía sau» | mistranslation | Medium | facts updated in the reference (security keys, up to seven; MetaMask 7702 and sponsorship; licence status C-lic-1; fixed network list) | rows retranslated from the 080 English |
| F-14 | `home.pricing.cards` | «Ứng dụng máy tính và di động — Miễn phí nếu tự biên dịch»; «Tải từ các cửa hàng ứng dụng — Mua một lần» | mistranslation | Medium | desktop is a free download; store apps are not available yet (C-plat-1) | «Web, tiện ích trình duyệt và máy tính — Miễn phí»; «iPhone và Android, tải từ cửa hàng — Mua một lần · sắp ra mắt» |
| F-15 | `home.networks.heading` / `.body` | «12 mạng dựng sẵn…»; link to the external biubiu tool | mistranslation + technical | High | C-net-1 (24); the count test and the href test both failed; href now `/chain-setup` | «24 mạng tích hợp sẵn. Thêm mạng của riêng bạn»; «trang thiết lập chuỗi cho biết thiếu những gì…» |
| F-16 | `home.faq.items[0].a` | «Một thiết bị mở khóa bằng Face ID hoặc vân tay, hoặc một khóa bảo mật USB/NFC.» | mistranslation | Medium | C-auth-1/C-keys-2; one security key alone can't create a wallet — missing "two, if only security keys" and "up to seven" | «…hoặc khóa bảo mật phần cứng — cần hai chiếc nếu bạn chỉ dùng khóa bảo mật… tối đa bảy…» |
| F-17 | `home.faq.items[1].a` | «Cách này chạy trong ứng dụng iOS, Android, máy tính và trong tiện ích…» | mistranslation | Medium | C-dapp-1: implies Linux desktop, omits that the web wallet doesn't connect to dApps | «…ứng dụng máy tính (macOS, Windows), iPhone và Android. Ví web không kết nối với dApp.» |
| F-18 | `home.faq.items[4].a` | «Ai chạm được vào passkey đã đồng bộ của bạn thì cũng có thể chạm được vào ví.» | mistranslation + unnatural | Medium | missing "a key can't be removed — move your funds to a new wallet" (C-compromise-1); *chạm được vào* ("can touch") is a literal figure | «…mà khóa thì không gỡ bỏ được. Hãy chuyển tiền sang một ví mới…» |
| F-19 | `home.faq.items[5].a` | «Vela không thể chuyển hay đóng băng tiền của bạn… Khóa công khai, địa chỉ ví và tên ví của bạn là công khai» | mistranslation | Medium | bare custody claim (C-custody-1: "by itself", and it writes the software that asks your keys to sign); incomplete public data (C-reg-1); missing what services see | retranslated in full |
| F-20 | `home.faq.items[6].a` | «…hãy ký từ trình duyệt: tiện ích Vela, **hoặc tiện ích ký minh bạch không phụ thuộc gì**» | mistranslation | High | offers the unpublished signing page as a way in (C-signpage-1, C-rp-1); drops the relay caveat (C-selfhost-2) | «…tiện ích trình duyệt Vela vẫn ký được… và các ứng dụng bạn tự biên dịch cũng vậy…» |
| F-21 | `about.lede` / `about.team.bio` | «chiếc ví, **các hợp đồng thông minh**, và chính trang này» | mistranslation | Medium | contradicts C-acct-1 (no contract in the funds path is Vela's) | «các ứng dụng, dịch vụ phía sau và chính trang web này…» |
| F-22 | `about.values[0..2]` | «Chúng tôi thay nó bằng passkey: khuôn mặt hoặc vân tay của bạn.»; values[0] «Khóa của bạn, coin của bạn…» | mistranslation | Medium | passkey equated with biometrics (C-auth-1, C-keys-2); values[0] lacked "what we do control is the software you sign with" (C-custody-1) | retranslated |
| F-23 | `roadmap.upcoming` | «…và **một cuộc kiểm toán bảo mật độc lập** cho phần tích hợp Safe + WebAuthn của Vela»; «một đường ký cho các chuỗi không có precompile P-256»; «tài khoản và mạng của bạn đã đi theo bản sao lưu của nền tảng» | mistranslation | High | promised an audit (forbidden: none is scheduled, A02 FR-2), a P-256 fallback (C-p256-1) and account sync (C-sync-1) | the five new items |
| F-24 | `roadmap.shipped` | 7 old items (e.g. «từ một mã nguồn duy nhất cũng build được cho iOS và Android») | technical + mistranslation | Medium | English is a new 10-item array; the React Native codebase the old text describes is retired | the ten new items |
| F-25 | `getStarted.platforms.desktop.stores` | «Mac App Store · Microsoft Store» | mistranslation | Medium | Vela is not on the Mac App Store; English lists Microsoft Store only | «Microsoft Store» |
| F-26 | `getStarted.fundingNote` | «…bạn luôn có thể tự build lấy — **cùng một ứng dụng**, không mất phí.» | mistranslation | Medium | a self-built phone app can't use the phone's own passkey (C-rp-1) — it is not the same app | «…ứng dụng điện thoại tự biên dịch sẽ ký bằng một điện thoại khác hoặc khóa bảo mật, chứ không dùng passkey của chính điện thoại đó.» |
| F-27 | `getStarted.meta.description` / `.lede` / `.platforms.web.blurb` | «…bản máy tính, di động và tiện ích trình duyệt đều dựng từ cùng một mã nguồn»; «Không phải cài, cũng không phải cập nhật…» | mistranslation | Medium | didn't say the extension and desktop download now and phones are still coming (C-plat-1); web blurb omitted that the web wallet doesn't connect to dApps (C-dapp-1) | retranslated from the 080 English |
| L-1 | `home.faq.items[2].a` | «iCloud Keychain hoặc Google Password Manager» | terminology | Low | the OS's own Vietnamese names, as the docs use | «Chuỗi khóa iCloud hoặc Trình quản lý mật khẩu của Google» |
| L-2 | `about.values[0].title` | «Tự quản, theo đúng nghĩa» | terminology | Low | *tự lưu ký* is the self-custody term (see table) | «Tự lưu ký, theo đúng nghĩa» |
| L-3 | `compare` "Mạng tùy chọn", "Độ chín" | «Mạng tùy chọn»; «Độ chín» | terminology / unnatural | Low | *tùy chọn* = optional; *độ chín* reads as fruit ripeness | «Mạng tùy chỉnh»; «Độ trưởng thành» |
| L-4 | `getStarted.sourceCta`, `chrome.docs.titles["clear-signing-self-host"]` | «Tự build từ mã nguồn»; «Tự chạy trang ký» | terminology | Low | consistency with *tự biên dịch* and *tự triển khai* | «Tự biên dịch từ mã nguồn»; «Tự triển khai trang ký» |
| L-5 | `chrome.docs.ui` (draft) | «Xem tài liệu» / «Ẩn tài liệu»; «← Trước» / «Tiếp →» | UI fit | Low | the toggle opens the list of pages, not the docs; bare *Trước/Tiếp* is terse for a pager label | «Xem danh mục tài liệu» / «Ẩn danh mục tài liệu»; «← Trang trước» / «Trang sau →»; «Chỉnh sửa trang này trên GitHub» |
| D-1 | docs/introduction | «**12 mạng**, một địa chỉ»; «bạn ký bằng passkey, bằng khuôn mặt hoặc vân tay»; «Nếu bạn mở khóa được điện thoại, bạn dùng được Vela.» | mistranslation | High | C-net-1, C-auth-1, C-keys-2 | rewritten as the new six-line summary and answer table |
| D-2 | docs/install | «không có gì để tải và không phải qua chợ ứng dụng nào»; «**Ứng dụng di động gốc sắp ra mắt**» as the only other option | mistranslation | High | C-plat-1 (extension and desktop are downloads today; stores are paid and not live) | rewritten with the four-platform table, extension steps, desktop notes |
| D-3 | docs/create-wallet | «Vela suy ra địa chỉ từ **khóa công khai của passkey**»; one-passkey flow; private key only in iCloud/Google | mistranslation | High | C-addr-1 (all founding keys), C-keys-1, C-keys-2 | rewritten: five steps, up to seven keys, "what is public" list |
| D-4 | docs/passkeys | «Đây chính là công nghệ bảo vệ Apple Pay»; «chỉ dùng được bằng khuôn mặt hoặc vân tay»; «Vela phát giao dịch đã ký lên mạng» | mistranslation | Medium | unsupported claim; C-auth-1; the relay submits, not the app | rewritten with the key-kind table and the "signs whatever you approve" warning |
| D-5 | docs/signers | «đăng ký một YubiKey ngay khi tạo ví và ký bằng nó» (as the only key); no section on a compromised key | mistranslation | Medium | a single unsynced key can't create a wallet (two security keys); C-compromise-1 missing | rewritten, incl. the per-app key table and "Nếu một khóa có thể đã bị lộ" |
| D-6 | docs/recovery | recovery = platform keychain only; advice «Đăng nhập trên nhiều hơn một thiết bị» | mistranslation | Medium | signing in on a second device adds no key; the reference's remedy is creating the wallet with more than one key; registry fallback and Ethereum copy missing | rewritten |
| D-7 | docs/networks-and-fees | 12 networks; «kích hoạt một tài khoản gas riêng… khoản đặt cọc kích hoạt không hoàn lại»; «**Không có nút chọn tốc độ**»; fee = «chi phí mạng cộng phí dịch vụ» | mistranslation | High | C-net-1, C-fee-1, C-fee-2 (no gas account exists), C-fee-3 (speed picker exists) | rewritten from the 080 page |
| D-8 | docs/send-and-receive | «Vela viết lại nó thành một số hữu hạn và từ chối gửi bất kỳ lệnh duyệt nào vẫn còn không giới hạn»; «hỏi passkey của bạn (Face ID / Touch ID / vân tay)» | mistranslation | Medium | C-approve-1 (permits and large finite approvals are not blocked); C-auth-1 | rewritten, incl. split/sweep, address names, fee coin and speed |
| D-9 | docs/clear-signing | «Duyệt chi không giới hạn bị chặn… viết lại yêu cầu thành một số hữu hạn»; «Vela không bao giờ mặc định coi là 18» | mistranslation | High | C-approve-1 without the permit / large-finite / `setApprovalForAll` caveat; the decimals claim is the opposite of current behaviour (shown as 18, marked unverified) | rewritten, incl. descriptor lookup order and "verified ≠ cryptographically checked" (C-clear-1) |
| D-10 | docs/bybit-attack | «**Không có hợp đồng nào chúng tôi nâng cấp được**»; «bỏ đi cái khả năng nâng cấp mà cuộc tấn công dựa vào»; «Sinh trắc học mới cho mỗi chữ ký» | mistranslation | High | false: the owner-signed `delegatecall` primitive Bybit used still exists in every Safe; the self-call gap (C-selfcall-1) was missing; C-auth-1 | rewritten; old phrasing kept where still true |
| D-11 | docs/account-contract | «bất kỳ giao diện tương thích Safe nào cũng điều khiển được»; «tốn khoảng 1,5–3× gas của một giao dịch EOA» | mistranslation | High | C-safeui-1 (reading yes, signing needs a getvela.app signature); measured cost is 140k–170k vs 21k gas; audit sentence covered only app code (C-audit-1) | rewritten with the five-contract table |
| D-12 | docs/security-audits | «Rà soát lần cuối: tháng 8/2026»; «Có bốn lớp hợp đồng»; no Certora M-01, no EntryPoint < v0.9 interception, no gaps list | mistranslation | High | stale review date; open findings the reader must know were absent | rewritten from the 080 page |
| D-13 | docs/clear-signing-self-host | no status line; «không tự phát yêu cầu mạng nào»; «bên tin cậy» | mistranslation + terminology | High | C-signpage-1 (built, not published, no app sends requests); it does load token logos; *bên tin cậy* = "trusted party", not relying party | rewritten with the status paragraph; *bên phụ thuộc (relying party)* |
| D-14 | docs/why-vela, Callout title | «Điều này không mang lại cho bạn thứ gì» | mistranslation | Medium | says "this brings you nothing" — the opposite of the point (extra keys help against loss, not theft) | «Điều mà khóa thêm không làm được» |
| D-15 | docs/why-vela | «bạn có thể chọn chính khóa đầu tiên là một khóa bảo mật phần cứng»; «để chiếc ví của bạn không bao giờ phụ thuộc vào việc công ty chúng tôi còn online» | mistranslation | Medium | a security-key-only wallet needs two keys; self-hosting stated without the passkey-domain limit (C-rp-1); the Callout linked "Safe smart account" to security-audits instead of account-contract | retranslated; link fixed |
| D-16 | docs/whitepaper | «tài khoản relay riêng (tài khoản gas)… kích hoạt lại»; «Mọi thứ đều mã nguồn mở theo giấy phép MIT — cả bốn dịch vụ»; «Trang web dùng công cụ phân tích **tự vận hành**»; 12 networks | mistranslation | High | C-fee-2, C-lic-1, C-net-1; the site loads a third-party analytics script (privacy fact) | rewritten, incl. the full threat model |
| D-17 | docs/faq | «bốn dịch vụ phía sau… đều công khai… theo giấy phép MIT»; «khoản đặt cọc nhỏ không hoàn lại để kích hoạt tài khoản relay gas»; 12 networks | mistranslation | High | C-lic-1, C-fee-2, C-net-1 | rewritten |

Counts: **23 High** and **21 Medium** found in the old text, all fixed; 5 notable Low
fixed. `self-hosting.md` is new (no old text to review): all six anchors, both
Callouts, every table and every code block kept; only code comments translated.

## Open items

- **"passkey" vs the OS term.** Vietnamese iOS calls passkeys *Mã khóa* and Google's
  Vietnamese UI says *Khóa truy cập*; the site (059) and the wallet UI both say
  *passkey*, and Vietnamese crypto media use the English word. Kept as *passkey*
  everywhere, per the brief; switching would be a file-wide and app-wide decision.
- **Split / sweep labels.** No vi UI string for the two batch modes was found in the
  wallet corpus, so the docs name them descriptively (**Chia**, **Gom**). If the app
  ships labels, the doc should quote them.
- **"chain" in the app's field labels.** The Service Endpoints fields say *chỉ mục dữ
  liệu chain* while the site says *dữ liệu chuỗi*. The self-hosting guide quotes the
  app label so readers can find the field; aligning the app wording is outside this
  locale's files.
- **Readiness.** This pass was a reading by the session that wrote the text, not by a
  native Vietnamese speaker; the 059 note that a native read is still owed applies.

## Result
reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the en + zh revision of the same day (fee wording, configurable relay chain
directory, hero subtitle, facts #3 and #4) into vi. Each changed string checked on the
five single-string axes; no High or Medium left open.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | «Việc ký được thực hiện ngay trên thiết bị của bạn. Khóa riêng tư của passkey không bao giờ được gửi tới Vela.» | — | *khóa riêng tư* as `passkeys.md` says; *diễn ra* → *được thực hiện* follows en "happens" → "is done" |
| `home.hero.facts[2]` | «Bạn ký đúng những gì bạn thấy: trước khi bạn duyệt, Vela giải mã chính giao dịch sẽ được ký.» / «…và cách Vela cho bạn thấy mình đang ký gì» | — | Vietnamese has no settled WYSIWYS term. *Thấy gì, ký nấy* (to match the callout «Thấy bao nhiêu, trả bấy nhiêu») was rejected: *thấy gì … nấy* also reads "whatever you see, you sign", the opposite of the point |
| `home.hero.facts[3]` | term = 059 string; link «Cách tiếp tục dùng ví nếu Vela không còn nữa» | — | 059 «Dù Vela ngừng hoạt động, bạn vẫn truy cập được ví của mình.» **kept**: same meaning as en and zh, natural. F-5 against it predates the en reinstatement; the overclaiming 059 link is not restored |
| `home.tradeoffs.items[0].body` | paragraph 2 rewritten (one fee to the relay, Vela's unless changed; formula + `#fee` link *cách tính*; relay pays the gas and keeps the rest); paragraph 3 tail aligned with en (", và khi đó…") | — | both hrefs identical to en |
| `home.faq.items[6].a` | code-change clause replaced by the four services you can run | — | second paragraph untouched |
| `roadmap.upcoming[1].body` | chain-data clause removed | — | |
| docs `networks-and-fees` | `<span id="fee">`; "gấp mười lần" paragraph replaced; **Ai nhận phí.** paragraph | — | |
| docs `faq` | cost bullet (relay choice, `#fee` link); shutdown answer without the code-change parenthesis | — | |
| docs `whitepaper` | intro clause dropped; Fees bullet + new "who gets the fee" bullet; «Nếu Vela biến mất» clause dropped | — | |
| docs `self-hosting` | intro limit dropped; `VELA_RELAY_CHAIN_DIRECTORY_URL` comment lines in both code blocks; **Cần biết** bullet; chain-data paragraph; relay line removed from the final list | — | *danh mục chuỗi* / *bản của Vela* as the file already said; "tháng 9/2026"; older builds = *các phiên bản relay cũ hơn*, so *bản* keeps meaning "copy" |

Nothing fixed beyond the brief.

## Update 2026-09-22 (audience)

Carried the en + zh audience revision (commit 66a3c789: copy for the reader who self-hosts
and builds from source; p256-index now MIT) into vi. Each changed string checked on the five
single-string axes; no High or Medium left open. `messages.test.ts -t "vi"` passes.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.meta.description` / `.ogDescription` / `.organization` | build-and-host framing (*tự biên dịch và tự triển khai*); organization *tự lưu ký* → *tự triển khai được* | — | en dropped "self-custodial" here, so *tự lưu ký* goes with it |
| `home.hero.ctaSelfHost` (new) | «Hoặc tự triển khai toàn bộ hệ thống» | — | inserted after `ctaCode`; "stack" rendered as *toàn bộ hệ thống* rather than the loanword |
| `home.why.more` | «Bản dài hơn — những điều chúng tôi không chấp nhận được ở các ví khác, và sự đánh đổi mà chúng tôi đã chọn» | — | *sự đánh đổi* as the introduction already says |
| `home.compare.rows` | reordered to the new en order; Source code cell → «…công khai</a>, tất cả theo giấy phép MIT» | — | anchor unchanged |
| `home.faq.items` | new order; three new answers and the merged lose-a-key/account answer | — | reused *Chuỗi khóa iCloud* / *Trình quản lý mật khẩu của Google*, «Không cần cụm từ khôi phục, email hay số dư ban đầu», «mà khóa thì không gỡ bỏ được»; *ngân quỹ*, *bundler* as the docs say |
| `getStarted.lede` | + «Ứng dụng nào cũng có thể tự biên dịch từ mã nguồn.» | — | |
| docs `introduction` | opening now *ví mã nguồn mở, tự triển khai được*; «Chạy được khi không có chúng tôi» bullet first and rewritten; table reordered with the new relay/fee row | — | |
| docs `faq`, `self-hosting`, `whitepaper` | all MIT including the index; (Rust, MIT); licence-file sentence deleted | — | |

Nothing fixed beyond the brief.

## Update 2026-09-22 (P-256 naming)

Added the two-names sentence to `networks-and-fees` («Precompile này có hai tên: … (tháng 12/2025) … trên các rollup») after the signature-check sentence.
Read every `EIP-7951 / RIP-7212` line in the docs and `home.networks.body`: "precompile EIP-7951 / RIP-7212" reads as a name after the noun, so no change was needed.
