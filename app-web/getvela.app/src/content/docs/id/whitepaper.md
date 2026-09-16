---
title: Whitepaper
description: Bagaimana Vela bekerja dan apa yang harus — dan tidak harus — Anda percayai untuk memakainya. Arsitektur, model keamanan, pemulihan, dan cara memverifikasi semuanya sendiri.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Status: alfa · v0.1">
Halaman ini menjelaskan bagaimana Vela bekerja hari ini dan apa yang harus — dan tidak
harus — Anda percayai untuk memakainya. Ia mengutamakan kejujuran di atas pemasaran.
Vela berada dalam tahap <a href="/blog/vela-is-in-alpha">alfa</a> — mulailah dengan
jumlah kecil. Vela tidak punya token. Semua yang ada di sini bisa diverifikasi terhadap
kode sumber terbukanya.
</Callout>

## Ringkasan

Vela adalah **dompet kontrak pintar swakelola** untuk jaringan EVM. Setiap dompet
adalah akun pintar [Safe](https://github.com/safe-fndn/safe-smart-account) yang
dikendalikan sebuah **passkey** — kredensial WebAuthn (P-256) yang dipegang sistem
operasi perangkat Anda, terenkripsi ujung-ke-ujung, dan dibuka dengan Face ID, Touch
ID, atau sidik jari. Tidak ada frasa pemulihan dan tidak ada kunci privat yang harus
Anda salin, simpan, atau hilangkan.

Vela, perusahaannya, tidak pernah memegang kunci atau dana Anda dan **tidak bisa
memindahkan, membekukan, atau menyitanya**. Aplikasinya, relay transaksinya, dan
layanan pendukungnya semuanya bersumber terbuka dan bisa Anda hosting sendiri. Yang
harus Anda percayai menyusut menjadi: kontrak pintar yang sudah diaudit, brankas
passkey sistem operasi Anda, dan — hanya untuk ketersediaan layanan — sebuah relay yang
bisa Anda ganti atau jalankan sendiri.

## Kenapa Vela ada

Kebanyakan dompet memaksa sebuah pertukaran:

- **Dompet berfrasa pemulihan** menaruh rahasia 12–24 kata di depan setiap pengguna. Ia
  titik kegagalan tunggal sekaligus sasaran phishing yang terus-menerus.
- **Dompet kustodian** menghapus frasa pemulihannya tetapi mengambil alih dana Anda —
  menghadirkan kembali risiko lawan transaksi yang justru ingin dihapus kripto.
- **Tanda tangan buta** — menyetujui hex tak terbaca — sudah jadi hal biasa di
  ekosistem ini dan berada di balik sebagian besar dompet yang terkuras.

Vela ingin semudah aplikasi kustodian sambil menjaga Anda sepenuhnya swakelola: tanpa
frasa pemulihan, tanpa kustodian, dan tanpa transaksi yang tidak bisa Anda baca sebelum
menandatanganinya.

## Prinsip desain

1. **Swakelola, tanpa pengecualian.** Kunci dibuat di perangkat Anda dan dipegang
   penyedia passkey sistem operasi Anda, terenkripsi ujung-ke-ujung. Server Vela hanya
   pernah melihat data publik.
2. **Verifikasi, jangan percaya begitu saja.** Seluruh tumpukannya — aplikasi dan
   keempat layanan pendukungnya — bersumber terbuka dengan lisensi MIT.
3. **Tanpa tanda tangan buta.** Transaksi diterjemahkan menjadi maksud yang bisa dibaca
   manusia di mana pun deskriptornya ada; panggilan yang tidak dikenal ditandai, bukan
   disembunyikan.
4. **Berbuat lebih sedikit.** Dompetnya menyimpan ETH dan ERC-20 serta terhubung ke
   dApp yang Anda pilih. Lebih sedikit kode untuk dipercaya, permukaan serangan lebih
   kecil.

## Arsitektur

```text
Aplikasi Vela (iOS / Android / Web, satu basis kode)
  • Passkey (WebAuthn P-256, penyedia passkey sistem operasi)
  • Penyusunan & penandatanganan UserOperation
  • Antarmuka clear signing (ERC-7730)
        │  UserOperation bertanda tangan
        ▼
Relay Vela (ERC-4337, bisa di-hosting sendiri)
  • Mengirim handleOps ke EntryPoint
  • Tidak bisa mengubah atau memalsukan transaksi Anda
        ▼
Rantai EVM
  EntryPoint v0.7 → akun pintar Safe
  Penanda tangan WebAuthn memverifikasi P-256 di rantai
```

### Model akun

Dompet Anda adalah akun pintar **Safe v1.4.1** (sebuah kontrak proxy) yang dijalankan
lewat abstraksi akun **ERC-4337** (EntryPoint v0.7) dengan **Safe 4337 Module** dan
sebuah **penanda tangan WebAuthn** sebagai pemilik akunnya.

Alamatnya **deterministik** dan **kontrafaktual**: ia dihitung dari kunci publik passkey
Anda lewat `CREATE2` sebelum transaksi apa pun dikirim, jadi Anda bisa menerima dana di
situ sebelum ia pernah dipasang. Akunnya memasang dirinya sendiri, dibayar dari
saldonya sendiri, pada transaksi pertama Anda.

### Kunci dan autentikasi

Autentikasi memakai **passkey WebAuthn** di kurva **P-256**. Kunci privatnya dibuat di
perangkat Anda dan dipegang, terenkripsi ujung-ke-ujung, oleh penyedia passkey sistem
operasi Anda (iCloud Keychain atau Google Password Manager), yang menyinkronkannya antar
perangkat Anda. **Server Vela hanya pernah melihat kunci publik Anda.** Menandatangani
selalu menuntut verifikasi biometrik yang baru — tidak ada kunci sesi berumur panjang.
Lihat [cara kerja passkey](/id/docs/passkeys) untuk detail lengkapnya.

### Penandatanganan dan alur transaksi

1. **Menyusun** `UserOperation` ERC-4337 untuk Safe Anda dan memperkirakan gasnya.
2. **Menerjemahkan** panggilannya menjadi maksud yang bisa dibaca manusia dan
   menampilkannya untuk ditinjau.
3. **Menandatangani** — perangkat Anda menghasilkan pernyataan WebAuthn atas digest
   operasinya setelah verifikasi biometrik.
4. **Mengodekan** pernyataan itu sebagai tanda tangan kontrak **EIP-1271**.
5. **Meneruskan** operasi bertanda tangan itu ke relay, yang mengirimkannya ke
   EntryPoint.
6. **Memverifikasi di rantai** — Safe memverifikasi tanda tangan P-256 di rantai lewat
   precompile RIP-7212 sebelum mengeksekusinya. Precompile itu syarat mutlak: tidak ada
   verifikator cadangan, dan Vela menolak mengaktifkan jaringan yang tidak punya itu.

Relay menerima operasi yang **sudah ditandatangani**. Ia tidak bisa mengubah penerima,
jumlah, atau kolom lain tanpa membatalkan tanda tangannya.

### Relay dan model gas

- Gas dibayar **dari saldo dompet Anda sendiri** — dengan token asli jaringan secara
  bawaan, atau dengan stablecoin yang didukung di tempat relay menawarkannya. Tempo,
  yang tidak punya koin asli, selalu menyelesaikan gas dengan stablecoin USD. Tidak ada
  **paymaster** dan tidak ada pihak ketiga yang mensponsori — atau menggerbangi —
  transaksi Anda.
- **Relay adalah satu-satunya sumber kebenaran untuk harga gas.** Ia memberi kuotasi
  dari kondisi rantai terkini; dompetnya menampilkan kuotasi itu dan menandatangani
  persis apa yang ditampilkannya.
- Tagihan relayer Vela sengaja dibuat sederhana: totalnya adalah **biaya jaringan
  ditambah biaya layanan relayer**, dengan tagihan minimum kecil untuk transaksi yang
  sangat murah. Sebagian mengalir ke validator rantainya; sisanya membayar relayer yang
  menjalankan infrastrukturnya dan menjaga akun gas Anda tetap terisi.
- Dompetnya **menampilkan perkiraan biaya sebelum Anda mengonfirmasi** — dalam aset
  biaya dan mata uang tampilan Anda — dan jumlah yang dikuotasi beserta penerimanya
  adalah bagian dari yang Anda tandatangani, jadi relayer dibayar persis sebesar yang
  ditampilkan. Tidak ada markup tersembunyi.
- Setiap Safe punya **akun relayer khusus** (akun gas) per rantai, yang diaktifkan
  dengan deposit yang **tidak dapat dikembalikan**. Akun itu bisa menipis seiring waktu,
  jadi ia mungkin perlu **diaktifkan ulang** nanti — jadi ia bukan benar-benar deposit
  sekali jalan.

Relay adalah ketergantungan **ketersediaan**, bukan ketergantungan **kustodi**: ia bisa
menunda atau menolak meneruskan, tetapi tidak pernah bisa mengubah, memalsukan, atau
mencuri. Ia bersumber terbuka dan Anda bisa menjalankan sendiri — dan karena harganya
**dikuotasi dan ditampilkan** alih-alih disembunyikan, bahkan biaya relay milik sendiri
atau milik pihak ketiga pun selalu terlihat oleh Anda sebelum menandatangani. Lihat
[jaringan & biaya](/id/docs/networks-and-fees).

### Clear signing (ERC-7730)

Vela menerjemahkan calldata dan data bertipe EIP-712 memakai deskriptor **ERC-7730** dan
menggambarkan **maksudnya** (Tukar, Kirim, Setujui…), **intinya** (jumlah, alamat), dan
**detailnya** (nonce, tenggat, calldata mentah) saat diminta, dengan kode warna menurut
risikonya. Saat tidak ada deskriptor yang cocok, Vela menampilkan peringatan tanda
tangan buta yang tegas alih-alih berpura-pura memahami panggilannya.

### Jaringan

Vela mendukung 12 jaringan EVM — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad, dan World Chain — ditambah jaringan
buatan sendiri. Jaringan buatan sendiri hanya bisa ditambahkan kalau ia sudah menampung
kontrak yang diandalkan Vela (EntryPoint, kontrak-kontrak Safe, penanda tangan WebAuthn)
dan precompile P-256 RIP-7212; Vela memeriksanya sebelum mengaktifkannya.

## Model keamanan

**Yang tidak bisa dilakukan Vela:**

- Memindahkan, membelanjakan, atau mentransfer dana Anda — hanya passkey Anda yang bisa
  memberi izin kepada Safe-nya.
- Membekukan atau menyita akun Anda — Safe itu kontrak Anda di rantai; Vela tidak punya
  peran istimewa padanya.
- Menandatangani atas nama Anda — setiap transaksi butuh pernyataan biometrik yang baru.
- Melihat kunci privat Anda — ia tidak pernah sampai ke Vela; hanya perangkat Anda yang
  bisa memakainya untuk menandatangani.
- Mengubah transaksi setelah Anda menandatangani — perubahan apa pun membatalkan tanda
  tangannya.

**Yang tidak dicakup oleh "tidak bisa membekukan":** *token*-nya. Stablecoin
berizin — USDC, USDT, dan sebagian besar token bersandar fiat — membawa fungsi daftar
hitam yang bisa dipanggil penerbitnya terhadap alamat mana pun, termasuk alamat Anda.
Kuasa itu milik penerbitnya dan ada apa pun dompet tempat Anda menyimpan tokennya; tidak
ada dompet swakelola, termasuk Vela, yang bisa mencabutnya. Yang diberikan swakelola
adalah: **kami** bukan pihak kedua yang juga bisa melakukannya.

**Yang Anda percayai:**

- **Kontrak Safe** (diaudit, dipakai luas) dan penanda tangan WebAuthn yang
  memverifikasi kunci P-256 Anda.
- **Penyedia passkey sistem operasi** Anda (Apple / Google) untuk melindungi dan
  menyinkronkan kredensial Anda.
- **Penyedia RPC** yang Anda kueri (Vela memakai kumpulan multi-sumber dengan
  pengalihan; Anda bisa memasang milik Anda sendiri).
- **Relay**, hanya untuk ketersediaan layanan — dan Anda bisa meng-hosting-nya sendiri.

**Ancaman yang dipertimbangkan:**

- **Perangkat hilang atau dicuri** — pencurinya tetap butuh biometrik/PIN Anda untuk
  menandatangani.
- **Phishing / dApp berbahaya** — ditangani dengan clear signing.
- **Server Vela yang dikuasai** — tidak memberi kemampuan menandatangani; radius
  dampaknya adalah layanan yang menurun, bukan kehilangan dana.
- **Risiko rantai pasok** — diperkecil oleh sumber terbuka dan hosting sendiri.

## Pemulihan

Passkey Anda dicadangkan oleh penyedia sistem operasi Anda; di perangkat baru, masuk
dengan akun Apple atau Google yang sama akan memulihkannya, dan dompet Anda muncul
kembali.

<Callout type="warning" title="Cadangan passkey platform Anda adalah pemulihan Anda">
Pemulihan di Vela adalah passkey Anda, yang disinkronkan iCloud Keychain atau Google
Password Manager. Secara desain tidak ada frasa pemulihan, tidak ada pemulihan sosial,
dan tidak ada wali — tidak ada yang bisa Vela hilangkan, bocorkan, atau dipaksa untuk
memakainya. Sisi lainnya nyata: kalau Anda kehilangan <strong>keduanya</strong> —
perangkat Anda <strong>dan</strong> passkey yang tersinkron di awan — tanpa salinan
lain, akunnya tidak bisa dipulihkan. Biarkan cadangan passkey platform Anda menyala dan
amankan akunnya.
</Callout>

Model pemulihan selengkapnya, termasuk batas-batas jujurnya, ada di
[pemulihan & masuk](/id/docs/recovery).

## Kalau Vela hilang

Swakelola berarti kunci dan dana Anda tidak bergantung pada Vela tetap online. Dana ada
di **kontrak Safe Anda sendiri di rantai**, dan relay-nya bersumber terbuka serta bisa
diganti.

Satu catatan jujur: WebAuthn mengikat passkey pada domain relying party
(`getvela.app`). Kalau domain itu hilang selamanya, passkey yang terikat padanya akan
butuh bantuan untuk bekerja di tempat lain — sebuah alat yang bisa menyodorkan relying
party aslinya kepada autentikator. Vela dulu menyediakan ekstensi browser kelas
pengembang untuk kasus itu dan menghentikannya pada September 2026; jalur pemulihan
kelas konsumen untuk kehilangan domain masih pekerjaan yang belum selesai, dan kami
mengatakannya alih-alih menyiratkan bahwa ia sudah ada. Akses on-chain secara independen
juga bergantung pada dukungan P-256 (RIP-7212) di rantai tujuan, yang terus membaik di
berbagai rantai.

## Privasi

Tidak ada akun, tidak ada email, tidak ada KYC, tidak ada frasa pemulihan untuk
dikumpulkan. Server hanya menyimpan **kunci publik** Anda dan nama akun yang Anda pilih
(untuk pemulihan lintas perangkat), yang memang dipublikasikan di rantai sesuai desain.
Isi transaksi tidak dicatat. Situsnya memakai analitik tanpa cookie yang di-hosting
sendiri. Lihat [kebijakan privasi](/privacy).

## Keterverifikasian dan sumber terbuka

Semuanya **berlisensi MIT dan bersumber terbuka** — aplikasinya dan keempat layanan
pendukungnya (data rantai, indeks passkey, relay, kurs mata uang), yang bisa Anda
**hosting sendiri** (Pengaturan → Lanjutan → Endpoint Layanan). Baca kodenya di
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

## Tanpa token

Vela **tidak punya token** dan tidak berencana membuatnya. Tidak ada yang bisa dibeli,
di-farming, atau dispekulasikan. Gas dibayar dengan aset asli masing-masing jaringan.

## Status audit dan keterbatasan

**Kontrak Safe** di inti setiap akun Vela diaudit secara independen dan sudah teruji
lapangan. **Integrasi Vela sendiri** di sekelilingnya **belum melewati audit pihak
ketiga yang independen**, dan belum ada yang dijadwalkan — audit profesional adalah
tujuan untuk saat proyek ini mampu membiayainya, bukan komitmen bertanggal. Sampai saat
itu, peninjauan integrasinya bersifat informal: kodenya terbuka, dan ia bersandar pada
anggota komunitas yang cakap dan tertarik untuk membacanya serta pada peninjauan
berbantuan AI. Itu membantu, tetapi tidak setara audit profesional. Perlakukan Vela
sebagai perangkat lunak alfa dan pakailah jumlah yang Anda nyaman taruh pada sesuatu
yang masih semuda ini.

## Rujukan

- ERC-4337 — Abstraksi akun lewat EntryPoint
- EIP-1271 — Validasi tanda tangan standar untuk kontrak
- ERC-7730 — Clear signing / deskriptor data terstruktur
- EIP-5792 — Pemaketan panggilan dompet
- RIP-7212 — Precompile verifikasi tanda tangan secp256r1 (P-256)
- WebAuthn / FIDO2 — Autentikasi passkey
- [Akun pintar Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
