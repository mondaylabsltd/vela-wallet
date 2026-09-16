---
title: Audit & masalah yang diketahui
description: Setiap kontrak on-chain yang diandalkan Vela, siapa yang mengauditnya, apakah versi yang diaudit sama dengan yang benar-benar terpasang, dan apa saja yang sama sekali belum diaudit.
---

"Diaudit" adalah klaim tentang versi tertentu dari kode tertentu, jadi halaman ini
tidak sekadar melambaikan kata itu — ia menyebut laporan yang persis, alamat deployment
yang persis, dan selisih antara versi yang diaudit dan versi yang terpasang. Ia juga
mendaftar apa yang _belum_ diaudit, karena daftar itu sama menentukannya dengan yang
pertama.

Terakhir ditinjau: Agustus 2026. Kalau Anda menemukan kekeliruan di sini, beri tahu
kami dan akan kami perbaiki.

## Jalur yang dilalui dana

Ada empat lapis kontrak yang bisa menyentuh uang Anda. Keempatnya kontrak pihak ketiga
dengan audit yang dipublikasikan, dan pada setiap kasus alamat yang terpasang adalah
deployment kanonik resminya.

### Safe v1.4.1 — akunnya sendiri

Dompet Anda adalah proxy
[Safe](https://github.com/safe-global/safe-smart-account): singleton SafeL2, proxy
factory, compatibility fallback handler, dan MultiSend untuk pemaketan.

[Ackee Blockchain mengaudit Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(laporan akhir Maret 2023): 11 temuan, tidak ada yang kritis atau tinggi. Versi v1.4.1
yang kami pasang berbeda dari v1.4.0 yang diaudit hanya pada satu baris perbaikan
kompatibilitas ERC-4337
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). Logika
MultiSend tidak berubah sejak
[v1.3.0 yang diaudit G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Semua alamat cocok dengan deployment kanonik di
[safe-deployments](https://github.com/safe-global/safe-deployments), dan kontraknya
masuk dalam
[program bug bounty Safe Foundation](https://docs.safefoundation.org/security/bug-bounty)
(hingga $1.000.000 untuk temuan kritis).

Satu hal yang tidak dicakup audit: insiden Bybit 2025. Serangan itu menguasai pipeline
build front-end web resmi Safe, bukan kontraknya —
[kesimpulan forensik resmi](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
tidak menemukan kerentanan pada kontrak pintar Safe. Kami membacanya sebagai pelajaran
tentang lapisan web dan operasional, yang juga lapisan tempat Anda seharusnya menyorot
kami.

### Safe4337Module v0.3.0 — adaptor ERC-4337

Terpasang di `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, alamat kanonik v0.3.0
(kecocokan persis di Sourcify — bytecode on-chain memang kode yang diaudit).
[Diaudit Ackee Blockchain](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(laporan akhir Maret 2024) tanpa temuan tak terselesaikan di atas taraf informasional.
Kombinasi v0.3.0 + EntryPoint v0.7 + Safe ≥1.4.1 yang kami pakai persis konfigurasi
yang digambarkan audit dan catatan rilisnya.

Riwayat modulnya memuat satu masalah yang pernah diungkap: v0.1.0 (2023) tidak
menandatangani `initCode` dan `paymasterAndData`, sebuah celah griefing gas. Itu
[diperbaiki di v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)
dan v0.1.0 tidak pernah keluar dari testnet. Kami memakai v0.3.0, yang mewarisi
perbaikannya.

### SafeWebAuthnSharedSigner v0.2.1 — penanda tangan passkey

Terpasang di `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, alamat kanonik v0.2.1 (sama
di setiap rantai lewat singleton factory milik Safe).

Apa arti "shared" — dan apa yang bukan: yang dibagikan adalah _deployment kontraknya_,
sebagaimana singleton Safe dibagikan. Kunci Anda tidak. Setiap Safe memanggil
`configure()` lewat delegatecall dan menyimpan kunci publik P-256-nya sendiri di
penyimpanannya sendiri. Satu instans penanda tangan mewakili tepat satu passkey per
Safe, dan Safe orang lain tidak bisa memakai milik Anda.

Versinya penting di sini. Audit v0.2.0
[secara eksplisit menyatakan](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
bahwa penanda tangan bersama itu di luar cakupan — kontraknya belum ada saat itu. Audit
yang mencakup apa yang kami pasang adalah audit v0.2.1:
[kompetisi audit Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(Juni–Juli 2024: nol tinggi, nol sedang, tiga temuan rendah — semuanya diperbaiki)
ditambah
[tinjauan Certora atas commit rilisnya](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
tanpa temuan baru. Sejak rilis belum ada kerentanan tingkat kontrak yang diungkap;
kontrak passkey-nya masuk cakupan bounty Safe Foundation.

Dokumentasi Safe sendiri menyarankan memasangkan kepemilikan lewat passkey dengan jalur
pemulihan, alih-alih memperlakukan satu kredensial sebagai satu-satunya kunci akun.
Bagaimana Vela menanganinya didokumentasikan di
[Pemulihan & masuk](/id/docs/recovery).

Verifikasi P-256 on-chain memakai precompile RIP-7212 secara langsung, tanpa
verifikator cadangan berbasis Solidity. Sebelum mengaktifkan jaringan apa pun,
aplikasinya menguji precompile itu dengan tanda tangan sungguhan dan menolak jaringan
tersebut kalau verifikasinya gagal. Dua catatan jujur: spesifikasi RIP-7212 aslinya
punya kelemahan pada kasus tepi yang justru menjadi alasan
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) ditulis (kelemahan itu tidak
memengaruhi tanda tangan WebAuthn yang terbentuk baik), dan sebuah pengujian tidak bisa
menangkap setiap cara implementasi sebuah rantai mungkin menyimpang dalam konteks
eksekusi yang tidak biasa.

### EntryPoint v0.7 — titik masuk ERC-4337

Terpasang di `0x0000000071727De22E5E9d8BAf0edAc6f37da032`,
[deployment kanonik v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Diaudit OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(atas permintaan Ethereum Foundation, Januari 2024): nol kritis, nol tinggi, lima
temuan sedang, semuanya diselesaikan — dan commit yang diaudit adalah rilis yang
terpasang. EntryPoint v0.7.0 masuk cakupan
[bug bounty ERC-4337](https://docs.erc4337.io/community/bug-bounty) milik Ethereum
Foundation (hingga $250.000).

## Masalah yang kami pantau

### Celah griefing di EntryPoint

Pada Februari 2026, para peneliti keamanan di Trust Security
[mengungkap](https://erc4337.substack.com/p/improving-useroperation-execution)
celah griefing dan sensor yang memengaruhi setiap EntryPoint sebelum v0.9, termasuk
v0.7 yang kami pakai. Penyerang yang menangkap UserOperation bertanda tangan sebelum ia
masuk blok bisa mengeksekusinya di dalam call frame yang ia kendalikan dan memaksa
eksekusi di dalamnya gagal — operasinya batal, tetapi gasnya tetap ditagih. Ethereum
Foundation membayar bounty $50.000 kepada penelitinya; mereka mengklasifikasikannya
sebagai celah sensor/griefing, bukan celah pencurian dana, dan celah itu tidak pernah
dieksploitasi.

Yang bisa dilakukannya: membakar satu biaya dan menunda satu transaksi. Yang tidak bisa:
mencuri dana atau memalsukan tanda tangan. Paparan Vela terhadapnya sempit karena
UserOperation langsung menuju relay alih-alih lewat mempool publik, jadi kesempatan
menangkapnya sedikit — dan kasus terburuknya dibatasi oleh biaya yang sudah Anda
setujui. Perbaikannya hanya ada di EntryPoint v0.9 (November 2025); v0.7 sendiri tidak
bisa ditambal. Kami berharap bisa bermigrasi seiring tumpukan di sekitarnya — terutama
lini modul 4337 milik Safe — menambahkan dukungan v0.9, dan akan kami catat di sini
saat itu terjadi.

## Apa yang belum diaudit

- **Kontrak buatan Vela sendiri.** Dua kontrak kecil yang kami tulis sendiri, terpasang
  di Gnosis:
  [indeks kunci publik passkey](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (registri append-only yang membantu perangkat Anda menemukan kunci publik Anda) dan
  kontrak pembantu batch-nya. Keduanya belum diaudit. Secara konstruksi keduanya tidak
  menyimpan dana, tidak punya pemilik, dan tidak bisa ditingkatkan — mereka lapisan
  penemuan, bukan lapisan otorisasi. Daya belanja selalu datang dari passkey yang
  dikonfigurasi di dalam Safe Anda. Kegagalan terburuk yang realistis adalah griefing
  (seseorang menyerobot sebuah entri indeks), yang bisa membuat pemulihan kurang praktis
  tetapi tidak bisa memindahkan uang. Kontrak pemecah penyelesaian gas dari desain biaya
  lama sudah tidak lagi menjadi bagian alur transaksi.
- **Multicall3.** README-nya sendiri
  [menyatakan terus terang](https://github.com/mds1/multicall3): "This contract is
  unaudited." Kami memakainya persis seperti yang penulisnya sebut aman — panggilan
  baca-saja yang dipaket untuk saldo, metadata token, dan kuotasi harga. Vela tidak
  pernah memberinya persetujuan dan ia tidak pernah menyimpan dana. Kasus terburuk dari
  sebuah bug adalah pembacaan yang keliru.
- **Deployer CREATE2.**
  [Proxy deployment deterministik Arachnid](https://github.com/Arachnid/deterministic-deployment-proxy)
  adalah deployer stateless standar ekosistem; ia tidak punya audit formal. Pemeriksaan
  jaringan kami gagal secara tertutup kalau ia hilang atau diubah di sebuah rantai.
- **Tempo dan pathUSD.** Tempo, salah satu dari dua belas jaringan bawaan kami, tidak
  punya koin asli; gas di sana diselesaikan dengan stablecoin pathUSD. Per Agustus 2026,
  baik protokol inti Tempo maupun pathUSD belum punya audit keamanan yang dipublikasikan
  atau program bug bounty, dan
  [penilaian jaminan independen dari DefiLlama](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (April 2026) menilai pathUSD berisiko tinggi. Ini risiko tingkat rantai yang tidak bisa
  diperkecil dompet mana pun: dana yang Anda simpan di Tempo, dan penyelesaian gas di
  sana, mewarisinya. Perlakukan Tempo sebagai rantai terbaru dan paling belum teruji
  dalam daftar ini dan sesuaikan besarnya saldo Anda. Kami akan memperbarui bagian ini
  seiring audit dipublikasikan.
- **Vela sendiri.** Aplikasi dan layanan pendukung kami belum melewati audit pihak
  ketiga. Itu catatan terbesar di halaman ini, kami menyatakannya di kepala situs, dan
  detail jujurnya ada di [Vela masih alfa](/blog/vela-is-in-alpha). Mulailah dengan
  jumlah kecil. Bacalah kodenya.

## Periksa sendiri

Setiap alamat di atas adalah deployment publik kanonik yang bisa Anda verifikasi
terhadap registri resmi —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
dan [catatan rilis EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Kontrak | Alamat |
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
| Indeks kunci publik passkey (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
