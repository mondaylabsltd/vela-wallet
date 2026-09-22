---
title: Audit & masalah yang diketahui
description: "Setiap kontrak yang diandalkan Vela, siapa yang mengaudit versi mana, apakah versi yang diaudit sama dengan yang di-deploy, temuan terbuka yang kami pantau, dan apa saja yang sama sekali tidak diaudit."
source: c4c50ad89f2f
---

"Sudah diaudit" adalah klaim tentang kode tertentu pada versi tertentu, jadi halaman ini
mengutip laporannya, commit-nya, dan alamat deployment-nya — dan mencantumkan apa yang
**tidak** diaudit, yang sama pentingnya.

Terakhir ditinjau: 22 September 2026. Kalau Anda menemukan kesalahan, beri tahu kami dan
kami akan memperbaikinya.

## Jalur dana

Setiap kontrak yang bisa menyentuh uang Anda adalah deployment kanonis dari kode pihak
ketiga dengan hasil tinjauan yang dipublikasikan.

### Safe v1.4.1 — akun itu sendiri

Dompet Anda adalah proxy [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
yang memakai singleton SafeL2 dan SafeProxyFactory. Transaksi gabungan (batch) berjalan
lewat MultiSend.

[Ackee Blockchain mengaudit Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(laporan final 16 Maret 2023, tinjauan perbaikan 28 Maret): 11 temuan, tidak ada yang
kritis atau tinggi; dua temuan sedang diakui, bukan diubah. Cakupannya SafeL2,
SafeProxyFactory, CompatibilityFallbackHandler, MultiSendCallOnly, dan SignMessageLib.
v1.4.1 berbeda dari v1.4.0 dalam satu baris fungsional, yaitu perbaikan kompatibilitas
ERC-4337 pada penyiapan modul
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); Safe
berkonsultasi dengan Ackee dan menyimpulkan audit ulang tidak diperlukan. Logika MultiSend
tidak berubah sejak v1.3.0, yang
[diaudit G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Semua alamat cocok dengan [safe-deployments](https://github.com/safe-global/safe-deployments).
Kontrak inti termasuk dalam cakupan
[program bug bounty Safe Foundation](https://docs.safefoundation.org/security/bug-bounty),
yang tingkat tertingginya membayar hingga $1.000.000.

Insiden Bybit tahun 2025 bukan temuan kontrak: penyerang memanipulasi JavaScript yang
disajikan ke antarmuka web Safe, dan
[pernyataan forensik](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
Safe tidak menemukan kerentanan di kontraknya. [Halaman kami tentang insiden itu](/id/docs/bybit-attack)
menjelaskan kenapa serangan jenis yang sama menyangkut setiap antarmuka dompet, termasuk
milik kami.

### Safe4337Module v0.3.0 — adaptor ERC-4337

Di-deploy di `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (exact match di Sourcify), dan
juga dipasang sebagai fallback handler Safe Anda. Ditinjau tiga kali —
[laporannya di sini](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, laporan final Maret 2024: satu peringatan (pemakaian optimizer
  compiler) diakui, tidak ada yang lebih tinggi yang masih terbuka.
- **Certora**, Agustus 2026: satu temuan **sedang**, diakui dan **tidak diperbaiki** di
  v0.3.0 — *perubahan otorisasi tidak membatalkan UserOperation berikutnya yang sudah
  divalidasi dalam bundle yang sama*. Lihat "Masalah yang diketahui" di bawah.
- **Nethermind**, Agustus 2026: tidak ada temuan.

SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), yang mengaktifkan modul saat dompet di-deploy,
tercakup dalam tinjauan Certora dan Nethermind.

Riwayat modul ini mencatat satu masalah yang pernah diungkap: v0.1.0 tidak
menandatangani `initCode` dan `paymasterAndData`, sebuah celah gas-griefing yang
[diperbaiki di v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
menurut Safe, v0.1.0 tidak dipakai di luar testnet. Vela memakai v0.3.0 dengan EntryPoint
v0.7 dan Safe 1.4.1, konfigurasi yang dijelaskan dalam rilis modul tersebut.

### Modul passkey Safe v0.2.1 — signer-nya

Kunci pertama Anda diverifikasi oleh **SafeWebAuthnSharedSigner** di
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`. "Shared" berarti deployment kontraknya
dipakai bersama, seperti singleton Safe; kunci Anda tidak. Setiap Safe menyimpan kunci
publik P-256-nya sendiri di storage-nya sendiri.

Setiap kunci tambahan punya kontrak signer sendiri, dibuat oleh
**SafeWebAuthnSignerFactory** di `0x1d31F259eE307358a26dFb23EB365939E8641195` sebagai
proxy ke **singleton SafeWebAuthnSigner** di
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`.

Tinjauan yang mencakup kontrak-kontrak ini pada v0.2.1
([laporan](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- [Kompetisi audit Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (Juni–Juli 2024): tidak ada temuan tinggi atau sedang; tiga temuan rendah, semuanya
  diperbaiki.
- Tinjauan **Certora** atas commit rilis: tidak ada temuan baru. (Audit v0.2.0 yang lebih
  awal mencatat bahwa shared signer saat itu belum diaudit — signer itu ditambahkan
  setelah audit tersebut.)
- **Nethermind**, Agustus 2026: tidak ada temuan.

Belum ada kerentanan tingkat kontrak yang diungkap sejak rilis, dan kontrak passkey
termasuk dalam cakupan bounty Safe Foundation.

Tanda tangan passkey diverifikasi oleh precompile **EIP-7951 / RIP-7212** milik chain, tanpa
verifier cadangan. Sebelum mengaktifkan sebuah jaringan, aplikasi memeriksa precompile
itu dengan tanda tangan sungguhan. Dua catatan: spesifikasi RIP-7212 yang asli punya
kelemahan pada kasus tepi yang diperbaiki [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)
(hanya memengaruhi input yang memang seharusnya gagal, bukan tanda tangan WebAuthn yang
formatnya benar), dan satu pengujian tidak bisa menangkap setiap kemungkinan implementasi
sebuah chain menyimpang.

### EntryPoint v0.7 — menjalankan operasi Anda

Di-deploy di `0x0000000071727De22E5E9d8BAf0edAc6f37da032`,
[rilis kanonis v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Diaudit oleh OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
untuk Ethereum Foundation (Januari 2024): tidak ada temuan kritis atau tinggi, lima
sedang, dan seluruh 24 temuan sudah diselesaikan; commit tinjauan perbaikannya cocok
dengan rilisnya. Kontrak ini termasuk dalam cakupan
[bug bounty ERC-4337](https://docs.erc4337.io/community/bug-bounty) milik Ethereum
Foundation (hingga $250.000).

## Masalah yang diketahui dan kami pantau

### Perubahan otorisasi dalam satu bundle (Safe4337Module, Certora M-01)

EntryPoint memvalidasi setiap operasi dalam satu bundle sebelum mengeksekusi salah
satunya. Jadi kalau satu operasi menghapus seorang pemilik, operasi yang ditandatangani
pemilik itu dan ditempatkan lebih belakang dalam bundle yang sama tetap lolos validasi
dan dijalankan. Safe mengakui hal ini dan tidak mengubah v0.3.0.

Aplikasi Vela tidak pernah menyusun penggantian pemilik, jadi Vela sendiri tidak pernah
memicu masalah ini. Meski begitu, masalah ini tetap penting: sebuah dApp bisa meminta
dompet Anda mengganti pemiliknya sendiri (lihat "Celah" di bawah), dan siapa pun yang
menghapus kunci yang bocor lewat alat Safe lainnya tidak bisa mengandalkan kunci itu
langsung terputus dalam bundle yang sama.

### Penyadapan operasi yang sudah ditandatangani (EntryPoint sebelum v0.9)

Pada Februari 2026, para peneliti
[mengungkap](https://erc4337.substack.com/p/improving-useroperation-execution) celah
griefing dan sensor yang memengaruhi setiap EntryPoint sebelum v0.9, termasuk v0.7.
Seseorang yang mendapatkan operasi yang sudah ditandatangani sebelum operasi itu ditambang
bisa mengeksekusinya di dalam panggilan yang ia kendalikan dan memaksa eksekusi di
dalamnya gagal (revert): operasinya gagal dan harus ditandatangani lagi. (Dengan biaya
in-band Vela, transfer biayanya ikut di-revert, jadi relay-lah, bukan Anda, yang
menanggung gasnya.) Celah ini memengaruhi operasi yang memanggil kontrak dengan proteksi
reentrancy atau yang bisa dibuat gagal oleh state sementara; transfer sederhana tidak
terpengaruh. Kalau dipakai berulang kali terhadap alur penarikan, celah ini bisa membuat
dana tidak bisa dipakai untuk sementara waktu. Celah ini tidak bisa memalsukan tanda
tangan atau mengalihkan dana.

Relay Vela mengirim operasi secara langsung, bukan lewat mempool bersama, tetapi transaksi
`handleOps` yang tertunda tetap terlihat di mempool publik, jadi ini hanya mempersempit
paparannya, bukan menghilangkannya. Perbaikannya hanya ada di EntryPoint v0.9 (November
2025); v0.7 tidak bisa di-patch. Migrasi bergantung pada dukungan modul 4337 milik Safe
untuk v0.9, dan halaman ini akan memberi tahu kapan itu terjadi.

### Celah dalam pertahanan Vela sendiri

Ini bukan temuan kontrak, melainkan tempat-tempat di mana dompet melindungi Anda lebih
sedikit daripada yang mungkin Anda kira. Masing-masing sudah dicatat untuk diperbaiki:

- **Panggilan dari dompet Anda ke dompet itu sendiri tidak diblokir.** Sebuah dApp bisa
  meminta `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, atau `setGuard`
  pada Safe Anda sendiri; salah satu saja, sekali ditandatangani, menyerahkan akun Anda.
  Vela mendekode panggilan ini tetapi tidak menghentikannya. Tolak setiap permintaan yang
  targetnya alamat Anda sendiri.
- **Pengaman persetujuan hanya menghentikan jumlah "tanpa batas"** (2^200 atau lebih;
  2^152 untuk Permit2). Persetujuan besar yang terbatas, permit yang ditandatangani, atau
  `setApprovalForAll` untuk NFT mendapat peringatan, bukan pemblokiran.
- **Deskriptor yang diambil tidak diautentikasi.** Deskriptor dari server data chain
  ditampilkan sebagai "terverifikasi" kalau cocok dengan kontraknya; tingkat
  kepercayaannya hanya setinggi server itu.
- **Halaman tanda tangan independen belum terhubung** ke aplikasi mana pun.
- **Pemeriksaan jaringan tidak mencari factory signer passkey milik Safe**, yang
  dibutuhkan kunci kedua sampai ketujuh; di jaringan yang ditambahkan tanpa factory itu,
  hanya kunci pertama yang bisa menandatangani.
- **Situs web memuat skrip analitik pihak ketiga** di domain yang sama dengan passkey.
  Situs itu melarang halamannya sendiri memakai passkey (lewat header Permissions-Policy),
  dan tidak memuat skrip tersebut di halaman yang memegang kunci.

## Apa yang tidak diaudit

- **Kontrak milik Vela sendiri.**
  [Registri kunci publik](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  di `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; alamat yang sama di Ethereum
  dan Base), deployment registri yang asli di
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (alamatnya adalah bagian dari domain tanda
  tangan setiap pendaftaran), dan indeks lama yang digantikan keduanya
  (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, riwayat hanya-baca). Kontrak-kontrak itu
  tidak diaudit. Kontrak itu tidak menyimpan dana, tidak punya pemilik, dan tidak bisa
  di-upgrade; fungsinya sebagai lapisan penemuan, bukan lapisan otorisasi. Kewenangan
  membelanjakan hanya berasal dari kunci yang dikonfigurasi di Safe Anda. Kegagalan
  terburuk yang realistis adalah dompet jadi lebih sulit ditemukan di perangkat baru,
  bukan uang yang berpindah.
- **Multicall3.** README-nya
  [menyatakan](https://github.com/mds1/multicall3) "This contract is unaudited." Vela hanya
  memakainya untuk pembacaan gabungan — saldo, detail token, kuotasi harga — tidak pernah
  untuk persetujuan atau dana.
- **Deployer deterministik** (proxy CREATE2 milik Arachnid dan singleton factory milik
  Safe) — standar di ekosistem dan tanpa state, tanpa audit formal. Pemeriksaan jaringan
  Vela langsung gagal (fail closed) kalau keduanya tidak ada; yang diperiksa adalah ada
  tidaknya kode di alamat itu, bukan kecocokannya byte demi byte.
- **Tempo.** Salah satu dari 24 jaringan bawaan, tanpa koin native; Vela membayar gas di
  sana dengan stablecoin pathUSD. Per September 2026,
  [kebijakan keamanan](https://github.com/tempoxyz/.github/blob/main/SECURITY.md) Tempo
  menyatakan protokolnya masih dalam proses audit dan tidak punya bug bounty yang aktif.
  Dana yang disimpan di Tempo, dan gas yang dibayar di sana, menanggung risiko tingkat
  chain itu; anggap Tempo sebagai chain terbaru dan paling belum teruji di daftar ini.
- **Vela sendiri.** Aplikasi, layanan backend, dan kontrak di atas belum pernah diaudit
  pihak ketiga, dan tidak ada audit yang dijadwalkan. Itulah catatan terbesar di halaman
  ini. Detailnya ada di [Vela is in alpha](/blog/vela-is-in-alpha). Mulailah dengan jumlah
  kecil, dan bacalah kodenya.

## Periksa sendiri

Setiap alamat di bawah ini adalah deployment publik yang kanonis. Verifikasi dengan
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments), dan
[rilis EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Kontrak                                    | Alamat                                       |
| ------------------------------------------ | -------------------------------------------- |
| Singleton SafeL2 v1.4.1                    | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                    | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                           | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹      | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                     | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                      | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1            | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1           | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| Singleton SafeWebAuthnSigner v0.2.1        | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                            | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Registri kunci publik (Vela, tidak diaudit) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Diperiksa saat jaringan ditambahkan; Safe Anda memakai modul 4337 sebagai fallback
handler-nya.
