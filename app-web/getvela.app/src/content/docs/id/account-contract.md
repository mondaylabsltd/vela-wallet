---
title: Kontrak akun
description: Dompet Vela Anda adalah Safe v1.4.1 tanpa modifikasi. Tidak ada satu baris pun di jalur kontraknya yang kami tulis — inilah yang Anda dapatkan, dan ongkosnya.
---

# Kontrak akun

Dompet Anda bukan struktur data internal sebuah aplikasi. Ia adalah akun pintar **Safe
v1.4.1** — kontrak yang sama yang menyimpan kas jauh lebih besar dari apa pun yang
akan pernah dilihat Vela — dipasang persis seperti yang dipublikasikan Safe, tanpa
modifikasi.

Kalimat itu pendek, konsekuensinya tidak, jadi halaman ini menuliskannya.

## Tidak ada yang di jalur itu milik kami

Ada empat kontrak yang berdiri antara Anda dan uang Anda. Vela tidak menulis satu pun:

| Kontrak | Siapa yang menulis |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (akunnya sendiri, sebuah proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (memverifikasi kunci P-256 Anda) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | Para penulis ERC-4337 |

Tidak ada kontrak buatan Vela. Repositorinya sama sekali tidak memuat Solidity — Anda
bisa memeriksanya dengan satu perintah:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # tidak mencetak apa pun
```

Saat Vela menambahkan jaringan, ia memasang **kontrak-kontrak itu** di alamat
kanoniknya. Ia tidak memasang kontrak rancangannya sendiri, dan ia tidak memegang
peran istimewa apa pun pada milik Anda: tidak ada kunci admin, tidak ada jalur
peningkatan, tidak ada modul yang bisa kami tambahkan.

## Kenapa "tanpa modifikasi" adalah kata yang menentukan

Banyak dompet dibangun di atas "sebuah Safe" atau "fork dari Safe" atau "akun yang
terinspirasi Safe". Fork adalah kontrak baru dengan reputasi lama. Bedanya, secara
praktis:

**Auditnya berlaku untuk hal yang benar-benar Anda pakai.** Laporan audit Safe
mencakup bytecode pada rilis persis ini. Audit sebuah fork mencakup kode sebelum fork
dilakukan. Kalau sebuah dompet memodifikasi kontrak akunnya, setiap audit yang
dikutipnya adalah audit atas sesuatu yang lain, dan modifikasinya justru bagian yang
tidak dilihat siapa pun.

**Ekosistem memperlakukan akun Anda sebagai Safe, karena memang begitu.** Penjelajah
blok bisa membacanya. Perkakas transaksi milik Safe sendiri memahaminya. Kalau besok
Vela hilang, dompet Anda bukan format yatim — ia akun pintar dengan dukungan perkakas
terluas di Ethereum, dan antarmuka apa pun yang kompatibel dengan Safe bisa
menjalankannya. Inilah yang membuat kalimat
["kalau Vela hilang, dompet Anda tidak"](/id/docs/why-vela) menjadi pernyataan tentang
kontrak, bukan tentang niat kami.

**Permukaan serangannya adalah permukaan yang juga diawasi semua orang.** Kontrak akun
buatan sendiri hanya diperhatikan penulisnya. Yang ini diperhatikan semua orang yang
menyimpan uang di dalam Safe.

## Ongkosnya

Menjadi standar tidak gratis, dan pertukarannya nyata:

- **Gas.** Akun pintar memverifikasi tanda tangan di rantai. Perkirakan sekitar 1,5–3×
  gas dari transfer EOA biasa, tergantung rantainya. Lihat
  [jaringan & biaya](/id/docs/networks-and-fees).
- **Akunnya harus dipasang.** Alamat Anda dihitung dengan `CREATE2` sebelum apa pun ada
  di rantai, jadi Anda bisa langsung menerima; tetapi transaksi keluar pertama membayar
  pemasangan kontraknya.
- **Tidak semua rantai memenuhi syarat.** Penanda tangan WebAuthn memverifikasi tanda
  tangan P-256 di rantai, dan itu memerlukan precompile **RIP-7212**. Vela lebih
  memilih menolak mengaktifkan jaringan yang tidak punya itu daripada turun ke
  verifikator yang lebih lemah.
- **Risiko Safe kini jadi risiko Anda.** Memercayai kontrak yang dipakai luas tetaplah
  memercayai sebuah kontrak. Yang bisa Vela katakan adalah kami tidak menambahkan hal
  kedua di atasnya untuk Anda percayai.

## Apa yang diaudit dan apa yang tidak

Kontrak Safe dan modul penanda tangan WebAuthn diaudit oleh pihak ketiga, dan
laporannya terbuka. **Kode aplikasi Vela sendiri belum diaudit secara independen**, dan
belum ada audit yang dijadwalkan — itu tujuan untuk saat proyek ini mampu membiayainya,
bukan komitmen bertanggal. Setiap kontrak yang diandalkan Vela, laporan auditnya, dan
masalah yang kami pantau terdaftar di
[audit & masalah yang diketahui](/id/docs/security-audits).

## Lihat sendiri

Akun Anda ada di rantai. Buka di penjelajah blok dan baca alamat implementasinya: ia
akan berupa deployment kanonik Safe v1.4.1, bita demi bita, di setiap jaringan yang
didukung Vela.
