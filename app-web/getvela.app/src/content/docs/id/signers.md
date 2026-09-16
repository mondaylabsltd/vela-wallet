---
title: Kunci penanda tangan & kunci keamanan
description: Dompet Vela bisa punya sampai tujuh kunci penanda tangan — passkey, perangkat di dekat Anda, atau kunci keamanan kelas YubiKey — dan salah satunya saja sudah bisa menandatangani. Semuanya ditentukan saat dompet dibuat, dan halaman ini menjelaskan kenapa itu bukan batasan yang lupa kami angkat.
---

# Kunci penanda tangan & kunci keamanan

Dompet Vela adalah sebuah Safe, dan Safe punya pemilik. Milik Anda bisa punya **sampai
tujuh**, dengan ambang **satu**: satu kunci penanda tangan mana pun bisa mengizinkan
transaksi sendirian. Itu ditulis `1-of-n`.

## Apa saja yang bisa jadi kunci penanda tangan

Tiga jenis, dan Anda bebas mencampurnya:

| Metode | Apa itu | Contoh umum |
| --- | --- | --- |
| **Platform** | Autentikator bawaan di perangkat yang sedang Anda pakai | Face ID / Touch ID di ponsel atau laptop ini, disinkronkan iCloud Keychain atau Google Password Manager |
| **Perangkat di dekat Anda** | Perangkat lain yang Anda jangkau dengan memindai kode | Ponsel Anda menandatangani untuk desktop, lewat transport hybrid WebAuthn |
| **Kunci keamanan** | Autentikator lepas-pasang lewat USB atau NFC | YubiKey dan kunci FIDO2 lainnya |

Ketiganya adalah kredensial WebAuthn di kurva **P-256**. Bagi Safe ketiganya tidak
bisa dibedakan: masing-masing adalah pemilik yang tanda tangannya diperiksa
verifikator WebAuthn on-chain dengan cara yang sama.

Kunci keamanan bisa jadi kunci penanda tangan **pertama** Anda, bukan sekadar
cadangan. Kalau Anda lebih suka dompet Anda sama sekali tidak bergantung pada akun
Apple atau Google, itulah pengaturan yang mewujudkannya — daftarkan YubiKey saat
pembuatan dan tandatangani dengan itu.

## Kenapa ditentukan saat pembuatan

Ini bagian yang mengejutkan banyak orang, jadi berikut mekanismenya, bukan permintaan
maaf.

Alamat dompet Anda **diturunkan** dari kumpulan pemiliknya. Vela menghitungnya dengan
`CREATE2` dari data penyiapan Safe — yang memuat kunci publik setiap penanda tangan —
sebelum apa pun dipasang di rantai. Itulah yang memungkinkan Anda menerima dana di
alamat yang belum ada.

Konsekuensinya aritmetika, bukan kebijakan: **kumpulan kunci yang berbeda adalah
alamat yang berbeda**. Menambah penanda tangan kedelapan belakangan tidak akan
memperluas dompet Anda; ia menghitung dompet baru, di alamat baru, tanpa uang Anda di
dalamnya.

Jadi pertanyaan "bisakah saya menambah kunci nanti?" punya dua jawaban jujur:

- **Sebelum Anda mengisinya**: bisa — alamatnya belum terikat pada apa pun, jadi buat
  ulang saja dompetnya dengan kunci yang Anda mau.
- **Setelah Anda mengisinya**: alamat itu tempat uang Anda berada. Mengubah pemilik
  pada Safe yang sudah terpasang adalah operasi Safe yang saat ini tidak dibuka Vela.
  Rencanakan kumpulan kuncinya sejak pembuatan.

## Ini sebenarnya melindungi Anda dari apa

**Kehilangan perangkat.** Dengan lebih dari satu penanda tangan, ponsel yang hilang
hanyalah kerepotan: kunci lain menandatangani. Dengan tepat satu penanda tangan dan
sinkronisasi sistem operasi dimatikan, ponsel yang hilang berarti dompet yang hilang —
itulah kenapa "passkey Anda tersinkron otomatis" adalah keterangan tentang pengaturan
yang Anda kendalikan, bukan jaminan yang bisa kami berikan atas nama Anda.

**Akun platform yang tidak lagi Anda percaya.** Kalau passkey Anda berada di iCloud
Keychain atau Google Password Manager, siapa pun yang menguasai akun itu berpotensi
memakainya. Kunci keamanan dipegang Anda sendiri dan tidak disinkronkan ke mana pun.

Dan yang **tidak** dilindunginya, karena `1-of-n` memotong ke dua arah: menambah kunci
kedua berarti menambah satu jalan *masuk* lagi, bukan gembok kedua. Siapa pun yang
mendapat salah satu kunci Anda bisa menandatangani sendirian. Lebih banyak kunci
berarti lebih tahan terhadap kehilangan dan lebih luas permukaan terhadap pencurian;
itulah pertukarannya, dan itu keputusan Anda.

## Memulihkan versus menambah

Ini dua hal berbeda dan dokumentasi menjaganya tetap terpisah:

- [Pemulihan & masuk](/id/docs/recovery) — kembali ke dompet yang sudah ada di
  perangkat baru dengan kunci yang sudah Anda miliki.
- Halaman ini — memutuskan sejak awal kunci mana saja yang ada.
