---
title: Whitepaper
description: "Bagaimana Vela bekerja dan apa yang perlu — dan tidak perlu — Anda percayai untuk memakainya: akun, kunci, biaya, model ancaman, pemulihan, dan apa yang terjadi kalau Vela menghilang."
source: 5bfc38a16ccb
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Status: alfa · terakhir direvisi September 2026">
Halaman ini menjelaskan bagaimana Vela bekerja saat ini dan apa yang perlu dan tidak
perlu Anda percayai untuk memakainya. Vela masih dalam tahap
<a href="/blog/vela-is-in-alpha">alfa</a> — mulailah dengan jumlah kecil. Vela tidak
punya token. Semua yang ada di sini bisa diperiksa terhadap kode open source-nya; kalau
kode dan halaman ini berbeda, kodelah yang benar, dan berarti ada bug di halaman ini.
</Callout>

## Ringkasan

Vela adalah **dompet kontrak pintar non-kustodial** untuk Ethereum dan jaringan EVM
lainnya. Setiap dompet adalah akun **Safe v1.4.1** yang tidak dimodifikasi, dijalankan
lewat **ERC-4337**, dan dikendalikan oleh hingga tujuh **passkey** — kunci WebAuthn
P-256 yang disimpan perangkat Anda, pengelola kata sandi Anda, atau kunci keamanan fisik.
Tidak ada frasa pemulihan.

Vela, sebagai perusahaan, tidak pernah memegang kunci Anda dan tidak punya peran di Safe
Anda, jadi Vela **tidak bisa memindahkan, membekukan, atau menyita dana Anda** secara
sepihak. Namun Vela memang menulis dan menyajikan perangkat lunak yang meminta kunci Anda
menandatangani — itulah sebabnya model ancaman di bawah ini penting. Aplikasinya, relay
yang mengirim transaksi, dan layanan pendukungnya open source, dan Anda bisa menjalankan
salinan sendiri dari masing-masing. Singkatnya, yang Anda percayai: kontraknya,
autentikator yang menyimpan kunci Anda, kode aplikasi yang Anda pakai untuk
menandatangani, domain tempat passkey Anda terikat, dan layanan yang Anda hubungkan ke
aplikasi.

## Kenapa Vela ada

- **Dompet berbasis frasa pemulihan** menyodorkan rahasia 12–24 kata kepada setiap
  pengguna: satu titik kegagalan dan sasaran phishing yang tak pernah hilang.
- **Dompet kustodial** menghilangkan frasa pemulihan dengan cara mengambil alih penyimpanan
  dananya.
- **Dompet passkey** yang bergantung pada server dan kode tertutup satu perusahaan memang
  menghilangkan frasa pemulihan, tetapi membuat Anda terlantar kalau perusahaan itu
  menghilang.
- **Tanda tangan buta** — menyetujui data buram yang tidak bisa Anda baca — masih umum,
  dan merupakan salah satu cara dompet dikuras.

Vela mengincar kenyamanan passkey tanpa satu pun ketergantungan itu: akun standar, kode
terbuka, layanan yang bisa diganti, dan transaksi yang bisa Anda baca sebelum Anda
menandatanganinya.

## Prinsip desain

1. **Non-kustodial, tanpa pengecualian.** Kunci dibuat dan disimpan oleh autentikator
   Anda. Layanan Vela tidak pernah melihatnya; apa yang memang mereka lihat tercantum di
   bagian Privasi.
2. **Kontrak standar, tanpa modifikasi.** Tidak ada satu pun kontrak di jalur menuju dana
   Anda yang ditulis Vela.
3. **Verifikasi, jangan percaya begitu saja.** Aplikasi dan layanannya terbuka untuk
   publik; layanannya bisa di-hosting sendiri.
4. **Dekode sebelum menandatangani.** Yang tidak bisa didekode membawa peringatan tanda
   tangan buta yang tegas.
5. **Lakukan lebih sedikit.** Dompet ini mengirim, menerima, dan menandatangani untuk dApp
   yang Anda pilih.

## Arsitektur

```text
Aplikasi Vela — web, ekstensi browser, desktop (macOS/Windows/Linux), iOS, Android
  satu inti Rust bersama (aturan, kripto, ABI, clear signing) + cangkang native tiap platform
  • menyusun UserOperation dan menunjukkan apa yang dilakukannya
  • meminta asersi WebAuthn dari kunci Anda
        │  UserOperation yang sudah ditandatangani (termasuk biaya)
        ▼
Relay (vela-relay, bisa di-hosting sendiri)
  • memberi kuotasi biaya, menalangi gas, mengirim handleOps
  • tidak bisa mengubah operasinya
        ▼
Chain EVM
  EntryPoint v0.7 → Safe v1.4.1 Anda → modul 4337 Safe
  modul passkey Safe memverifikasi P-256 lewat precompile EIP-7951 / RIP-7212
```

Layanan pendukung, semuanya open source: **indeks kunci publik** yang mendaftarkan dompet
baru di registri on-chain dan menjawab pencarian, direktori **data chain**, dan sumber
**kurs**. Lihat [panduan hosting sendiri](/id/docs/self-hosting).

### Akun

Dompet Anda adalah proxy **Safe v1.4.1** (singleton SafeL2), dengan **modul 4337 v0.3.0**
milik Safe diaktifkan sebagai modul dan fallback handler-nya, dan dijalankan lewat
**EntryPoint v0.7**. Pemiliknya adalah signer passkey dari **modul passkey v0.2.1**
milik Safe: kunci pertama diverifikasi oleh shared signer, dan setiap kunci tambahan oleh
kontrak signer-nya sendiri yang dibuat oleh factory milik Safe. Ambang batasnya **1**.

Alamatnya **deterministik dan kontrafaktual**: dihitung dengan `CREATE2` dari data
penyiapan Safe, yang mencakup setiap kunci awal, sebelum apa pun di-deploy. Alamat itu
sama di setiap jaringan. Anda bisa langsung menerima dana di sana; transaksi pertama Anda
di tiap jaringan men-deploy dompetnya dan membayarnya di dalam biaya transaksi itu.

### Kunci

Sebuah dompet punya **satu sampai tujuh kunci**, yang ditetapkan saat Anda membuatnya.
Salah satunya saja bisa menandatangani sendirian (1-of-n). Sebuah kunci bisa berupa:

- passkey di perangkat yang sedang Anda pakai — disinkronkan oleh Rantai Kunci iCloud,
  Pengelola Sandi Google, atau pengelola kata sandi lain jika Anda mengizinkannya;
- ponsel lain, yang dihubungkan dengan memindai kode QR (transport hybrid WebAuthn);
- kunci keamanan fisik lewat USB atau NFC, yang tidak tersinkron ke mana pun.

Setiap tanda tangan membutuhkan verifikasi pengguna dari autentikator itu sendiri —
biometrik atau PIN perangkat, atau PIN dan sentuhan pada kunci keamanan. Tidak ada
session key. Kunci tidak bisa ditambah, dihapus, atau diganti belakangan: di setiap
chain tempat dompet belum di-deploy, alamatnya tetap mewakili kumpulan kunci awal, jadi
mengganti pemilik di satu chain akan membuat akunnya berbeda dari chain ke chain.

Passkey milik sebuah relying party — passkey Vela dibuat untuk **`getvela.app`**.
Browser hanya menawarkannya ke halaman di getvela.app atau subdomainnya, dan itulah yang
membuat passkey tahan phishing; itu juga sebuah ketergantungan yang akan dibahas lagi di
bawah.

### Alur tanda tangan

1. **Susun** UserOperation untuk Safe Anda — termasuk transfer yang membayar relay — lalu
   simulasikan.
2. **Dekode** menjadi maksud yang bisa dibaca manusia dan tampilkan kepada Anda.
3. **Tanda tangani**: autentikator Anda membuat asersi WebAuthn atas hash operasi setelah
   memverifikasi Anda.
4. **Enkode** asersi itu menjadi tanda tangan Safe yang diharapkan modul passkey.
5. **Kirim** operasi yang sudah ditandatangani ke relay, yang memanggil EntryPoint.
6. **Verifikasi on-chain**: modul passkey memeriksa tanda tangan P-256 dengan precompile
   EIP-7951 / RIP-7212 sebelum Safe mengeksekusi apa pun. Tidak ada verifier cadangan; jaringan
   tanpa precompile itu tidak bisa ditambahkan.

### Biaya

- Relay dibayar **in-band**: operasinya mendeklarasikan biaya EntryPoint nol dan
  menyertakan transfer dari Safe Anda ke alamat relay. Jumlah dan penerimanya adalah
  bagian dari yang Anda tandatangani, jadi Anda membayar persis sebesar yang ditampilkan
  layar konfirmasi.
- Biayanya **tiga kali gas yang dicadangkan dompet untuk operasi itu** (perkiraan hasil
  simulasi dinaikkan setengahnya, dengan batas minimum), **dengan harga gas yang lebih
  tinggi antara hasil baca dompet sendiri dan harga relay untuk kecepatan yang dipilih**,
  dengan minimum sekitar $0,01. Di Tempo, pengalinya dua. Cadangan dan ruang lebih pada
  harganya membuat biaya itu berada di atas biaya on-chain operasi yang sebenarnya,
  terlebih untuk transaksi pertama di suatu jaringan; relay menyimpan selisihnya. Jumlah
  pastinya ada di layar konfirmasi sebelum Anda menandatangani.
- Biaya itu masuk ke relay yang dipakai dompet: relay Vela secara bawaan, atau
  deployment vela-relay mana pun, termasuk yang Anda jalankan sendiri.
- Biaya dibayar dengan koin jaringan itu atau dengan stablecoin USD yang diterima relay
  (pathUSD di Tempo, yang tidak punya koin native). **Tidak ada paymaster**: tidak ada
  yang mensponsori gas, dan tidak ada yang bisa menyaring transaksi lewat kebijakan
  sponsor.
- Kalau treasury gas milik relay di suatu jaringan kosong, dompet memberi tahu sebelum
  Anda menandatangani. Tidak ada deposit per pengguna.

Detailnya: [jaringan & biaya](/id/docs/networks-and-fees).

### Clear signing

Panggilan dan pesan EIP-712 didekode dengan deskriptor **ERC-7730** — bawaan aplikasi
untuk kontrak yang umum, diambil dari layanan data chain, atau dicocokkan dengan bentuk
token standar — lalu, sebagai jalan terakhir, basis data selector publik, dengan label
upaya terbaik (best effort). Sisanya mendapat peringatan tanda tangan buta yang tegas.
Deskriptor yang diambil tidak diautentikasi secara kriptografis. Persetujuan on-chain di
tingkat "tanpa batas" (2^200 atau lebih) tidak bisa dikirim sampai Anda menurunkannya;
persetujuan besar yang terbatas dan permit yang ditandatangani ditampilkan dengan
peringatan hati-hati tetapi tidak diblokir. Detailnya:
[clear signing](/id/docs/clear-signing).

### Jaringan

Vela punya 24 jaringan bawaan — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium,
MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume, dan XRPL EVM — dan menerima
jaringan EVM apa pun yang punya sebelas kontrak yang diperiksanya dan precompile
EIP-7951 / RIP-7212. (Kunci kedua sampai ketujuh juga membutuhkan factory signer passkey milik Safe
di jaringan itu, yang belum tercakup dalam pemeriksaannya.)

## Model keamanan

**Apa yang tidak bisa dilakukan Vela**

- Memindahkan, membelanjakan, atau membekukan dana Anda secara sepihak — hanya kunci Anda
  yang bisa mengotorisasi Safe Anda, dan Vela tidak punya peran di dalamnya. (Yang bisa
  dilakukan Vela adalah merilis perangkat lunak yang meminta Anda menandatangani; lihat
  ancaman di bawah.)
- Mengubah transaksi setelah Anda menandatanganinya — perubahan apa pun membuat tanda
  tangannya tidak sah.
- Membaca kunci privat Anda — kunci itu tetap di autentikator Anda.
- Menambahkan kunci ke dompet Anda, atau menghapusnya.

**Yang tidak tercakup oleh "tidak bisa membekukan": tokennya.** USDC, USDT, dan sebagian
besar token yang dijamin mata uang fiat mengizinkan penerbitnya memasukkan alamat mana pun
ke daftar hitam, termasuk alamat Anda. Kewenangan itu milik penerbit dan tetap ada apa pun
dompet yang Anda pakai. Yang diberikan dompet non-kustodial adalah bahwa Vela bukan pihak
kedua yang bisa melakukannya.

**Yang Anda percayai**

- **Kontraknya**: Safe, modul 4337 dan modul passkey-nya, EntryPoint v0.7, dan precompile
  EIP-7951 / RIP-7212 milik chain.
- **Domainnya**: halaman apa pun yang disajikan dari getvela.app atau salah satu
  subdomainnya bisa meminta tanda tangan dari kunci Anda.
- **Autentikator** yang menyimpan kunci Anda, dan — untuk passkey yang tersinkron — akun
  Apple, Google, atau pengelola kata sandi di baliknya.
- **Kode aplikasi yang Anda pakai untuk menandatangani.** Aplikasi itulah yang menyusun
  transaksi dan menunjukkan apa yang dilakukannya. Aplikasi yang dibobol bisa menampilkan
  satu hal dan meminta Anda menandatangani hal lain; permintaan konfirmasi dari
  autentikator tidak akan memberi tahu perbedaannya.
- **Endpoint RPC** tempat Anda membaca data: node yang berbohong bisa menampilkan saldo
  yang salah atau pratinjau simulasi yang salah. Anda bisa mengatur node sendiri.
- **Layanan data chain dan kurs**: keduanya menyediakan daftar token, deskriptor, daftar
  token untuk biaya, dan kurs yang dipakai untuk mengubah jumlah fiat menjadi jumlah
  token.
- **Relay**: relay tidak bisa mengubah apa yang Anda tandatangani, tetapi bisa menunda
  atau menolaknya, memilih kapan transaksi masuk ke chain (jadi relay bisa melakukan
  front-running atas sebuah swap dalam batas slippage Anda), dan menetapkan harga gas yang
  menjadi dasar biaya Anda, hingga tiga kali hasil baca dompet sendiri.

**Ancaman yang dipertimbangkan**

- **Perangkat hilang atau dicuri** — pencuri tetap harus lolos pemeriksaan
  autentikatornya; kunci lain memulihkan akses. Namun kunci tidak bisa dihapus: kalau
  salah satunya mungkin sudah di tangan orang lain, pindahkan dana Anda ke dompet baru,
  karena alamat lama tetap bisa dibelanjakan oleh kunci itu di setiap jaringan.
- **Phishing** — passkey tidak bisa diketik di situs palsu, dan browser hanya
  menawarkannya ke halaman di getvela.app dan subdomainnya.
- **dApp berbahaya** — ditangani dengan clear signing dan pengaman persetujuan, dengan
  satu celah serius: dApp bisa meminta panggilan dari Safe Anda ke Safe itu sendiri —
  `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard` — dan salah
  satu saja, sekali ditandatangani, menyerahkan akun Anda sepenuhnya, sama seperti payload
  Bybit. Vela mendekode panggilan ini tetapi belum memblokirnya. Tolak setiap permintaan
  yang targetnya alamat dompet Anda sendiri.
- **Layanan backend yang dibobol** (relay, indeks, data chain, kurs) — tidak punya
  kewenangan menandatangani, tetapi punya pengaruh nyata: menolak layanan, deskriptor atau
  daftar token yang menyesatkan, kurs yang salah sehingga mengubah berapa yang terkirim
  untuk suatu jumlah fiat, dan (untuk relay) waktu dan harga gas di atas. Deskriptor yang
  diambil tidak dianggap terautentikasi, dan setiap layanan bisa diganti.
- **Distribusi aplikasi yang dibobol** — deployment web, pembaruan ekstensi, atau build
  aplikasi yang dimanipulasi bisa menyodorkan transaksi berbahaya untuk Anda tandatangani.
  Ini jenis serangan [Bybit](/id/docs/bybit-attack). Mitigasinya saat ini terbatas: dekode
  dan pengaman persetujuan di aplikasi itu sendiri, build macOS yang dinotarisasi, dan
  mengompilasi ekstensi atau aplikasinya sendiri dari kode sumber (paket rilis disertai
  checksum SHA-256, bukan tanda tangan). Halaman tanda tangan independen yang tidak
  berbagi kode dengan aplikasi sudah dibuat, tetapi belum terhubung.
- **Apa pun yang disajikan dari domain itu** — halaman mana pun di getvela.app atau
  subdomainnya, termasuk skrip yang dimuatnya, bisa meminta tanda tangan dari passkey
  Vela, dan permintaan konfirmasinya hanya menampilkan "getvela.app". Karena itu situs web
  melarang halamannya sendiri memakai passkey, dan tidak memuat skrip analitiknya di
  halaman yang memegang kunci. Kalau domain itu berpindah tangan, pemilik barunya juga
  akan mengendalikan aplikasi mana yang boleh memakai passkey itu. Ekstensi dan aplikasi
  yang dikompilasi sendiri membawa kodenya sendiri, meskipun secara bawaan keduanya masih
  mengambil deskriptor dan memakai layanan di bawah getvela.app.

## Pemulihan

Pembuatan dompet memublikasikan kunci publik dan alamatnya ke **kontrak registri** publik
di Gnosis (bisa disalin ke Ethereum). Di perangkat baru, Anda masuk dengan **salah satu**
kunci; aplikasi menemukan dompetnya lewat indeks atau, kalau gagal, langsung dari
registri, lalu memeriksa bahwa kunci-kunci itu menghasilkan kembali alamat yang tercatat.
Dompet dengan satu kunci juga bisa dibangun ulang dari dua tanda tangan tanpa registri
sama sekali.

<Callout type="warning" title="Kunci Anda adalah pemulihan Anda">
Tidak ada frasa pemulihan, tidak ada pemulihan sosial, dan tidak ada guardian — tidak ada
yang bisa dihilangkan, dibocorkan, atau dipaksakan untuk dipakai oleh Vela. Kalau setiap
kunci awal hilang, dompetnya tidak bisa dipulihkan. Buat dompet dengan lebih dari satu
kunci, biarkan sinkronisasi passkey aktif kalau Anda mengandalkannya, dan amankan akun di
baliknya.
</Callout>

Detailnya: [pemulihan & masuk](/id/docs/recovery).

## Kalau Vela menghilang

Dana Anda tetap berada di Safe Anda on-chain. Kontraknya tidak bergantung pada Vela, dan
setiap layanan yang dijalankan Vela adalah open source sehingga bisa dijalankan pihak lain.
Satu-satunya yang tidak bisa dipindahkan adalah relying party passkey itu, `getvela.app`:
salinan dompet web di domain lain membuat dompet yang berbeda. Untuk dompet yang sudah
ada, ekstensi browser Vela (yang bisa memakai passkey `getvela.app` berdasarkan izin) dan
aplikasi yang Anda kompilasi sendiri (dengan ponsel atau kunci keamanan) tetap berfungsi
tanpa getvela.app.
[Panduan hosting sendiri](/id/docs/self-hosting#if-getvela-app-disappears) menguraikan
setiap jalur beserta batasannya. Akses mandiri ke sebuah chain juga mensyaratkan chain itu
mendukung EIP-7951 / RIP-7212.

## Privasi

Tanpa akun, tanpa email, tanpa KYC. Yang menjadi publik ditulis ke registri saat Anda
membuat dompet: kunci publik dan ID kredensial tiap kunci, model autentikator, nama dompet
dan label kunci Anda, alamatnya, dan data pendaftaran yang ditandatangani. Indeks Vela
melihat catatan itu sebelum mengirimkannya, begitu pula alamat-alamat yang Anda cari
namanya; relay Vela melihat alamat Anda, operasi yang Anda kirim, dan endpoint RPC yang
dipakai aplikasi Anda (termasuk kunci API apa pun di URL-nya), dan menyimpan operasi untuk
waktu terbatas guna mencoba ulang dan mendiagnosis masalah. Setiap layanan melihat alamat
IP Anda. Situs webnya memakai analitik tanpa cookie.
[Kebijakan privasi](/privacy) adalah daftar yang berlaku.

## Open source

Semuanya berlisensi MIT: dompetnya (semua aplikasi dan intinya), relay, indeks kunci
publik, layanan kurs, dan direktori data chain. Kodenya:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Tanpa token

Vela tidak punya token dan tidak berencana membuatnya. Tidak ada yang bisa dibeli,
di-farm, atau dispekulasikan. Biaya dibayar dengan koin tiap jaringan atau stablecoin.

## Status audit dan keterbatasan

Kontrak Safe, modul 4337 dan modul passkey-nya, serta EntryPoint v0.7 sudah diaudit
secara independen dan dipakai luas. **Kode Vela sendiri — aplikasi, layanan backend, dan
kontrak registri — belum pernah diaudit pihak ketiga yang independen, dan tidak ada audit
yang dijadwalkan**; audit profesional adalah sasaran untuk saat proyek ini mampu
membiayainya, bukan komitmen dengan tanggal. Sampai saat itu, tinjauannya bersifat
informal: kodenya terbuka, anggota komunitas yang cakap membacanya, dan kode itu ditinjau
dengan bantuan alat AI. Itu membantu; tetapi tidak setara dengan audit profesional.
Perlakukan Vela sebagai perangkat lunak alfa. Detailnya:
[audit & masalah yang diketahui](/id/docs/security-audits).

## Referensi

- ERC-4337 — Abstraksi akun lewat EntryPoint
- EIP-1271 — Validasi tanda tangan untuk kontrak
- ERC-7730 — Deskriptor clear signing
- EIP-5792 — Batching panggilan dompet (`wallet_sendCalls`)
- EIP-7951 / RIP-7212 — Precompile verifikasi tanda tangan P-256
- WebAuthn / FIDO2 — Passkey
- [Akun pintar Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
