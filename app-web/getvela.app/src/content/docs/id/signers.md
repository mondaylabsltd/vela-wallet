---
title: Kunci penanda tangan & kunci keamanan
description: "Dompet Vela bisa punya hingga tujuh kunci penanda tangan — passkey, ponsel di dekat Anda, atau kunci keamanan sekelas YubiKey — dan salah satunya saja sudah bisa menandatangani. Kunci-kunci itu dipilih saat dompet dibuat; halaman ini menjelaskan alasannya, dan apa yang harus dilakukan kalau sebuah kunci mungkin sudah bocor."
source: 072891bd4768
---

# Kunci penanda tangan & kunci keamanan

Dompet Vela adalah sebuah Safe, dan Safe punya pemilik. Dompet Anda bisa punya **hingga
tujuh** pemilik, dengan ambang batas **satu**: satu kunci penanda tangan mana pun bisa
mengotorisasi transaksi sendirian. Ini ditulis `1-of-n`.

## Apa saja yang bisa menjadi kunci penanda tangan

Ada tiga jenis, dan Anda bebas mencampurnya:

| Metode | Apa itu | Contoh umum |
| --- | --- | --- |
| **Platform** | Autentikator bawaan perangkat yang sedang Anda pakai | Face ID / Touch ID di ponsel atau laptop ini, disinkronkan oleh Rantai Kunci iCloud atau Pengelola Sandi Google |
| **Perangkat di dekat Anda** | Perangkat lain yang dihubungkan dengan memindai kode | Ponsel Anda menandatangani untuk komputer desktop, lewat transport hybrid WebAuthn |
| **Kunci keamanan** | Autentikator lepas-pasang lewat USB atau NFC | YubiKey dan kunci FIDO2 lainnya |

Ketiganya adalah kredensial WebAuthn pada kurva **P-256**. Bagi Safe, ketiganya tidak
bisa dibedakan: masing-masing adalah pemilik yang tanda tangannya diperiksa on-chain
dengan cara yang sama oleh modul passkey milik Safe. (Kunci pertama diverifikasi oleh
signer bersama milik Safe; tiap kunci tambahan oleh kontrak signer kecilnya sendiri,
yang dibuat oleh factory milik Safe saat dompet pertama kali di-deploy di suatu chain.)

Jenis kunci yang bisa dipakai tiap aplikasi:

| Aplikasi | Perangkat ini | Ponsel di dekat Anda (QR) | Kunci keamanan |
| --- | --- | --- | --- |
| Dompet web, ekstensi browser | Ya | Ya | USB atau NFC, lewat browser |
| Desktop (macOS, Windows, Linux) | macOS dan Windows | Ya | USB |
| Android | Ya (dengan layanan Google Play) | Ya | USB |
| iOS | Ya | Ya | YubiKey USB-C atau Lightning, firmware 5.8 atau yang lebih baru |

Opsi "Perangkat ini" di aplikasi desktop (Touch ID, Windows Hello), dan dukungan Windows
secara umum, masih baru dan belum seteruji jalur lainnya; untuk saat ini, ponsel atau
kunci keamanan adalah pilihan yang lebih andal di sana.

Kunci keamanan bisa menjadi kunci penanda tangan **pertama** Anda, bukan hanya cadangan.
Kalau Anda lebih suka dompet Anda sama sekali tidak bergantung pada akun Apple atau
Google, buat dompet dengan **dua** kunci keamanan dan simpan salah satunya di tempat
aman. (Dompet yang satu-satunya kuncinya tidak tersinkron ke mana pun tidak bisa dibuat:
aplikasi meminta kunci kedua, karena kehilangan satu perangkat itu berarti kehilangan
dompetnya.)

## Kenapa dipilih saat pembuatan

Bagian ini sering mengejutkan orang, jadi berikut mekanismenya, bukan permintaan maaf.

Alamat dompet Anda **diturunkan** dari kumpulan pemiliknya. Vela menghitungnya dengan
`CREATE2` dari data penyiapan Safe — yang mencakup kunci publik setiap kunci penanda
tangan — sebelum apa pun di-deploy on-chain. Itulah yang memungkinkan Anda menerima dana
di alamat yang belum ada.

Untuk alamatnya, konsekuensinya sekadar aritmetika: **kumpulan kunci yang berbeda berarti
alamat yang berbeda**. Menambah kunci penanda tangan belakangan tidak akan memperluas
dompet Anda; itu akan menghitung dompet baru, di alamat baru, tanpa uang Anda di
dalamnya.

Jadi pertanyaan "bisakah saya menambah kunci belakangan?" punya dua jawaban jujur:

- **Sebelum Anda mengisinya dengan dana**: bisa — alamatnya belum terikat pada apa pun,
  jadi buat saja dompet itu lagi dengan kunci yang Anda inginkan.
- **Setelah Anda mengisinya dengan dana**: alamat itulah tempat uang Anda berada. Safe
  sendiri bisa mengganti pemilik di chain tempat dompet Anda sudah di-deploy — tetapi di
  setiap chain yang belum di-deploy, alamat yang sama tetap mewakili kunci-kunci awal,
  sehingga kumpulan pemilik di tiap chain lama-lama tidak lagi sama. Menjaganya tetap
  selaras di semua chain memang mungkin — sebagian dompet pintar melakukannya — tetapi
  Vela belum membangunnya, jadi Vela tidak menyediakan penggantian pemilik. Rencanakan
  kumpulan kunci Anda saat pembuatan.

## Apa yang sebenarnya dilindunginya

**Kehilangan perangkat.** Dengan lebih dari satu kunci penanda tangan, ponsel yang hilang
hanya merepotkan: kunci lain tetap bisa menandatangani. Dengan tepat satu kunci dan
sinkronisasi sistem dimatikan, ponsel yang hilang berarti dompet yang hilang — itulah
sebabnya "passkey Anda tersinkron otomatis" adalah gambaran sebuah pengaturan yang Anda
kendalikan, bukan jaminan yang bisa kami berikan atas nama Anda.

**Akun platform yang tidak lagi Anda percayai.** Kalau passkey Anda tersimpan di Rantai
Kunci iCloud atau Pengelola Sandi Google, siapa pun yang menguasai akun itu berpotensi
memakainya. Kunci keamanan dipegang oleh Anda sendiri dan tidak tersinkron ke mana pun.

Dan apa yang **tidak** dilindunginya, karena `1-of-n` punya dua sisi: menambah kunci
kedua berarti menambah jalan *masuk* kedua, bukan gembok kedua. Siapa pun yang
mendapatkan salah satu kunci penanda tangan Anda bisa menandatangani sendirian. Makin
banyak kunci berarti makin tahan terhadap kehilangan dan makin luas celah untuk
pencurian; itulah kompromi yang harus diputuskan, dan keputusannya ada di tangan Anda.

## Kalau sebuah kunci mungkin sudah bocor

Kunci tidak bisa dihapus. Kalau salah satu kunci Anda mungkin sudah berada di tangan
orang lain — ponsel tak terkunci yang hilang, kode sandi yang terlihat orang, akun Apple
atau Google yang tidak lagi Anda kuasai — **pindahkan semuanya ke dompet baru** yang
dibuat dengan kunci yang Anda percayai. Alamat lama tetap bisa dibelanjakan oleh kunci
itu di setiap jaringan, termasuk dana yang dikirim siapa pun ke alamat itu di kemudian
hari.

## Memulihkan, bukan menambah

Keduanya hal yang berbeda, dan dokumentasi ini memisahkannya:

- [Pemulihan & masuk](/id/docs/recovery) — kembali ke dompet yang sudah ada di perangkat
  baru dengan kunci yang sudah Anda punya.
- Halaman ini — memutuskan, sejak awal, kunci mana saja yang ada.
