---
title: Pengantar
description: Apa itu Vela, untuk siapa, dan gagasan di balik dompet pintar swakelola tanpa frasa pemulihan.
---

# Pengantar

Vela adalah **dompet pintar swakelola** untuk jaringan EVM. Kunci itu milik Anda,
tetapi tidak ada frasa pemulihan yang harus dicatat — Anda menandatangani dengan
passkey, memakai wajah atau sidik jari Anda.

Dokumentasi ini membahas cara memulai, membuat dompet, memindahkan token, dan
memahami model keamanan di baliknya.

## Versi singkat

- **Swakelola.** Dana Anda dikendalikan oleh kunci yang hanya Anda yang bisa
  memakainya. Vela (perusahaan) tidak bisa memindahkan, membekukan, atau memulihkan
  uang Anda.
- **Tanpa frasa pemulihan.** Kunci penanda tangan Anda adalah passkey di perangkat
  keras aman milik perangkat Anda. Tidak ada dua belas kata yang bisa hilang atau
  dicuri lewat phishing.
- **Akun pintar Safe.** Setiap dompet adalah kontrak
  [Safe](https://github.com/safe-fndn/safe-smart-account) yang dijalankan dengan
  abstraksi akun ERC-4337 — itulah yang memungkinkan Anda menandatangani dengan
  passkey dan membaca setiap transaksi sebelum menyetujuinya.
- **12 jaringan, satu alamat.** Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
  Base, Avalanche, Gnosis, Unichain, Tempo, Monad, dan World Chain — ditambah jaringan
  buatan sendiri — semuanya di alamat yang sama.
- **Tanda tangan yang bisa dibaca.** Transaksi diterjemahkan menjadi maksud yang bisa
  dibaca manusia (ERC-7730) bila deskriptornya tersedia; kalau tidak, Vela
  menerjemahkan sebisanya dan menampilkan peringatan. Panggilan yang tidak bisa dibaca
  ditandai, bukan disembunyikan.
- **Sumber terbuka.** Dompet dan semua layanannya
  [terbuka di GitHub](https://github.com/mondaylabsltd/vela-wallet) sehingga siapa pun
  bisa memeriksa apa sebenarnya yang mereka lakukan.
- **Perangkat lunak alfa.** Vela berjalan dan menyimpan dana sungguhan, tetapi belum
  melewati bertahun-tahun penempaan di lapangan. Mulailah dengan jumlah kecil.
  [Tulisan soal alfa](/blog/vela-is-in-alpha) menjelaskan artinya.

## Untuk siapa

Vela dibuat untuk orang yang ingin swakelola sungguhan tanpa jebakan mengelola frasa
pemulihan — dan untuk orang yang pernah kena batunya. Kalau Anda bisa membuka kunci
ponsel, Anda bisa memakai Vela.

## Berikutnya ke mana

- [Memasang Vela](/id/docs/install) — berjalan di browser, tidak perlu mengunduh apa pun.
- [Membuat dompet Anda](/id/docs/create-wallet) — dompet pertama dalam sekitar satu menit.
- [Cara kerja passkey](/id/docs/passkeys) — model keamanannya, dijelaskan apa adanya.
- [Whitepaper](/id/docs/whitepaper) — arsitektur dan model kepercayaan selengkapnya.

Kalau Anda lebih tertarik pada *kenapa* daripada *bagaimana*, [blog](/blog)
menceritakan bagaimana Vela dibangun.
