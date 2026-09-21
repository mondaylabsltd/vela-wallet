---
title: Serangan Bybit, dan jalur yang dipakainya
description: "Pada Februari 2025 Bybit kehilangan sekitar $1,5 miliar. Yang jebol bukan kontrak Safe — melainkan antarmukanya. Halaman ini menjelaskan jalurnya, dan bagian mana dari desain Vela yang menutupnya."
source: ac56b16b531f
---

# Serangan Bybit, dan jalur yang dipakainya

Pada 21 Februari 2025, Bybit kehilangan sekitar **$1,5 miliar** dari cold wallet
multisig Safe. Ini pencurian terbesar dalam sejarah industri ini, dan layak dibaca
dengan cermat, karena hampir semua hal di dalamnya *benar* — kecuali satu.

## Apa yang terjadi

Versi singkatnya, dari laporan pascainsiden (post-mortem) yang dipublikasikan:

1. Penyerang membobol **komputer seorang developer `Safe{Wallet}`** dan menyisipkan
   JavaScript berbahaya ke bucket AWS S3 yang menyajikan front-end `Safe{Wallet}`. Kode
   itu dimasukkan pada 19 Februari dan dipicu pada 21 Februari, dengan sasaran khusus
   Safe milik Bybit.
2. Para penanda tangan Bybit membuka antarmukanya dan meninjau transaksi yang tampak
   biasa saja.
3. Payload yang sebenarnya dikirim ke **dompet perangkat keras** mereka bukanlah
   transaksi itu. Payload itu adalah `delegatecall` yang menimpa `masterCopy` milik
   proxy Safe — slot 0 — dan mengganti seluruh implementasi akun dengan milik penyerang.
4. Para penanda tangan menyetujuinya. Tanda tangannya sah. Kontraknya melakukan persis
   apa yang diperintahkan.

Secara publik, serangan ini dikaitkan dengan aktivitas yang terhubung dengan Korea Utara
(FBI menyebut kelompok TraderTraitor).

## Apa yang *tidak* jebol

- **Bukan kontrak Safe.** Kontrak itu mengeksekusi instruksi yang ditandatangani secara
  sah. Tidak ada bug di Safe yang dieksploitasi.
- **Bukan kriptografinya.** Setiap tanda tangan asli.
- **Bukan dompet perangkat kerasnya.** Perangkat Ledger ikut dalam prosesnya dan tetap
  menandatangani — karena dompet perangkat keras menampilkan apa yang diberikan
  kepadanya, dan yang diberikan kepadanya adalah payload berbahaya itu. Perangkat yang
  tidak bisa mendekode `delegatecall` menjadi sesuatu yang bisa dinilai manusia
  melindungi *kuncinya*, bukan *keputusannya*.

Yang jebol adalah asumsi yang mendasari setiap antarmuka dompet: **bahwa layar yang
menggambarkan sebuah transaksi dan byte yang ditandatangani adalah hal yang sama.**

## Kenapa ini kasus umum, bukan kejadian langka

Sebagian besar tanda tangan yang dibuat di dompet web bertumpu pada asumsi itu.
Antarmukanya menyusun payload, antarmukanya merender ringkasan, dan tidak ada pihak
independen yang memeriksa bahwa keduanya cocok. Kalau kode yang menyajikan antarmuka itu
diganti — lewat pipeline build yang dibobol, CDN yang dibajak, dependensi berbahaya,
kredensial deploy yang dicuri — ringkasannya menjadi apa pun yang diinginkan penyerang,
sementara tanda tangan Anda asli.

Inilah risiko yang menjadi sasaran desain tanda tangan Vela. Bukan phishing. Bukan kunci
yang bocor. **Layar tanda tangan yang membohongi Anda.**

## Apa yang dilakukan Vela

**Clear signing, sampai ke calldata.** Setiap transaksi didekode menjadi maksud yang bisa
dibaca manusia sebelum Anda menyetujuinya — jumlah, penerima, apa yang sebenarnya
dilakukan panggilan itu ([ERC-7730](/id/docs/clear-signing)). Panggilan yang tidak bisa
kami dekode **ditandai sebagai tidak bisa didekode**, bukan diam-diam dirender seolah
tidak ada masalah. Payload Bybit adalah `delegatecall` yang menukar alamat implementasi;
bentuk seperti itulah yang semestinya membuat penanda tangan langsung berhenti, dan
menyembunyikannya di balik ringkasan yang ramah adalah alasan hal itu tidak terjadi.

Ada dua batasan yang perlu dijelaskan dengan tepat. dApp tidak bisa meminta Vela
melakukan `delegatecall` secara langsung — permintaan yang bisa dibuat sebuah halaman
hanya menghasilkan panggilan biasa — jadi payload Bybit itu sendiri tidak bisa masuk
lewat jalur ini. Tetapi sebuah halaman *bisa* meminta panggilan dari Safe Anda ke Safe
itu sendiri: `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`.
Salah satu saja dari panggilan itu, sekali ditandatangani, menyerahkan akun Anda
sepenuhnya, sama seperti payload Bybit — modul yang sudah diaktifkan kemudian bisa
menjalankan `delegatecall`-nya sendiri. Vela mendekode panggilan semacam itu tetapi belum
memblokirnya; **tolak setiap permintaan yang targetnya alamat dompet Anda sendiri.** Dan
kalau kode Vela sendiri diganti, seperti yang terjadi pada `Safe{Wallet}`, hasil dekodenya
pun akan menjadi milik penyerang — untuk itulah poin berikutnya.

**Jalur independen yang bisa memeriksa antarmuka.** Vela sudah membuat
[halaman tanda tangan](/id/docs/clear-signing-self-host) tanpa langkah build dan tanpa
dependensi, yang mendekode permintaan dan melakukan tanda tangan WebAuthn secara mandiri —
satu folder berisi file statis yang bisa Anda baca dari awal sampai akhir, sajikan
sendiri, atau muat sebagai ekstensi browser. Tujuannya menjadi pendapat kedua yang tidak
berbagi rantai pasok (supply chain) dengan aplikasi utama. *Status: sudah dibuat dan
diuji; belum dipublikasikan, dan belum ada aplikasi Vela yang mengirim permintaan ke
sana.* Halaman ini akan menyatakannya dengan jelas saat hal itu berubah.

**Kami tidak punya peran admin yang bisa direbut.** Akun Vela adalah
[Safe v1.4.1 yang tidak dimodifikasi](/id/docs/account-contract), dan Vela tidak memegang
peran istimewa apa pun di dalamnya — tidak ada kunci admin dan tidak ada jalur upgrade
milik kami yang bisa kami pakai karena dipaksa atau dibobol. Perlu jelas juga apa yang
*tidak* dihilangkan oleh hal ini: primitif yang dipakai penyerang Bybit — `delegatecall`
yang ditandatangani pemilik dan menulis ulang implementasi akun — tetap ada di setiap
Safe, termasuk milik Vela (transaksi gabungan Vela sendiri memakai `delegatecall` ke
MultiSend milik Safe). Primitif itu membutuhkan tanda tangan sah dari salah satu kunci
Anda. Pertahanan agar Anda tidak terbujuk memberikannya adalah dekode di atas dan
pemeriksaan independen.

**Pemeriksaan baru untuk setiap tanda tangan.** Setiap tanda tangan membutuhkan
konfirmasi dari kunci Anda sendiri — Face ID, sidik jari, PIN, atau sentuhan dan PIN di
kunci keamanan. Tidak ada session key yang berlaku lama, jadi tidak ada celah waktu ketika
sesuatu bisa menandatangani atas nama Anda tanpa kehadiran Anda.

**Hosting sendiri sebagai benteng terakhir.** Aplikasi dan layanan backend-nya open
source. Kalau Anda sama sekali tidak ingin memercayai pipeline build kami, kompilasi
sendiri ekstensi atau aplikasinya dan jalankan layanan yang Anda butuhkan —
[panduan hosting sendiri](/id/docs/self-hosting) menjelaskan caranya. Itu mengeluarkan
pipeline build kami dari rantai kepercayaan; Anda tetap memercayai kode yang Anda
kompilasi, jadi bacalah kodenya.

## Apa yang tidak diklaim Vela

Front-end Vela bisa dibobol dengan cara yang sama seperti front-end `Safe{Wallet}`. Kode
kami tidak diaudit. Mengatakan sebaliknya justru merupakan jenis jaminan yang semestinya
sudah berakhir sejak insiden ini.

Yang coba dilakukan desainnya adalah mempersempit jalurnya: membuat payload bisa dibaca
alih-alih buram, tidak memegang peran admin yang bisa disalahgunakan terhadap Anda, dan
memberi Anda cara untuk memverifikasi dengan sesuatu yang bukan kami. Ringkasan jujurnya:
**serangan jenis ini dimitigasi lewat desain, bukan dihilangkan**, dan bagian-bagian yang
akan memperkuatnya lebih jauh tercantum, belum selesai, di
[audit & masalah yang diketahui](/id/docs/security-audits).

## Sumber

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
