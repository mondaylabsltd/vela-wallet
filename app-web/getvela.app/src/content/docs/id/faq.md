---
title: Tanya jawab
description: "Jawaban singkat tentang kustodi, kunci, pemulihan, jaringan, biaya, apa yang bisa dilihat Vela, open source, dan apa yang terjadi kalau Vela tidak ada lagi."
source: 7446e22f990d
---

# Tanya jawab

## Apakah Vela dompet non-kustodial?

Ya. Dompet Anda adalah akun pintar Safe yang hanya dikendalikan oleh kunci Anda, dan
kunci itu tetap berada di perangkat, pengelola kata sandi, atau kunci keamanan Anda. Vela
tidak memegang kunci apa pun dan tidak punya peran di dompet itu, jadi Vela tidak bisa
memindahkan, membekukan, atau memulihkan dana Anda secara sepihak. Namun, perangkat lunak
yang meminta kunci Anda menandatangani memang ditulis oleh Vela — lihat
[model ancaman](/id/docs/whitepaper).

## Benar-benar tidak ada frasa pemulihan?

Benar. Kunci Anda adalah passkey, dan passkey tidak punya rahasia yang bisa Anda catat
atau ketik. Lihat [cara kerja passkey](/id/docs/passkeys).

## Apa yang saya butuhkan untuk membuat dompet?

Perangkat yang mendukung passkey (ponsel atau komputer yang cukup baru dengan Face ID,
sidik jari, atau Windows Hello), atau dua kunci keamanan fisik. Tanpa email, tanpa akun,
tanpa saldo awal. Anda bisa membuat dompet dengan hingga tujuh kunci; kunci tidak bisa
ditambah belakangan. Lihat [membuat dompet Anda](/id/docs/create-wallet).

## Bagaimana kalau ponsel saya hilang?

Masuk di perangkat baru dengan kunci lain mana pun: passkey yang sama yang tersinkron
lewat Rantai Kunci iCloud atau Pengelola Sandi Google, ponsel lain, atau kunci keamanan
Anda. Kalau ponsel itu menyimpan satu-satunya kunci Anda dan kunci itu tidak tersinkron,
dompetnya tidak bisa dipulihkan. Lihat [pemulihan & masuk](/id/docs/recovery).

## Jaringan dan token apa saja yang didukung?

24 jaringan EVM bawaan, termasuk Ethereum, Base, Arbitrum, Optimism, Polygon, BNB Chain,
Gnosis, dan Avalanche, ditambah jaringan EVM mana pun yang Anda tambahkan selama memenuhi
persyaratannya. Koin native dan token ERC-20. Alamatnya sama di setiap jaringan. Lihat
[jaringan & biaya](/id/docs/networks-and-fees).

## Berapa biayanya?

- **Aplikasinya:** dompet web, ekstensi browser, dan aplikasi desktop gratis. Aplikasi iOS
  dan Android akan dijual sebagai pembelian sekali bayar di toko aplikasi; Anda juga bisa
  mengompilasi aplikasi mana pun dari kode sumber secara gratis.
- **Setiap transaksi:** biaya yang dibayar dari dompet Anda ke relay yang mengirimkannya
  — relay Vela, kecuali Anda mengarahkan dompet ke relay lain atau menjalankan relay
  sendiri. Biaya ini mencakup gas plus margin relay, dengan minimum sekitar $0,01. Jumlah
  pastinya ada di layar konfirmasi sebelum Anda menandatangani dan merupakan bagian dari
  yang Anda tandatangani. Tidak ada deposit dan tidak ada langganan.
  [Cara biaya dihitung](/id/docs/networks-and-fees#fee).
- **Tanpa token.** Vela tidak punya token dan tidak berencana membuatnya.

## Bisakah saya memakai Vela dengan dApp?

Bisa, lewat ekstensi browser Vela (Chrome, Edge, Brave) dan browser bawaan aplikasi
desktop (macOS, Windows), iOS, dan Android. Dompet web di wallet.getvela.app tidak
terhubung ke dApp. Lihat [memasang Vela](/id/docs/install#dapps).

## Apa yang bisa dilihat atau dilakukan Vela?

Vela tidak bisa membaca kunci Anda atau memindahkan dana Anda secara sepihak. Layanannya
melihat alamat IP Anda dan apa yang ditanyakan aplikasi kepadanya: indeks melihat kunci
publik dan nama dompet Anda saat mendaftarkan dompet baru, serta alamat yang Anda cari;
relay melihat alamat Anda, operasi yang Anda kirim, dan endpoint RPC yang dipakai aplikasi
Anda; layanan data chain melihat token dan kontrak mana yang ditanyakan aplikasi Anda. Apa
yang menjadi publik on-chain tercantum di
[membuat dompet Anda](/id/docs/create-wallet#what-is-public).
[Kebijakan privasi](/privacy) adalah versi lengkap yang berlaku.

## Apakah Vela open source?

Ya, semuanya berlisensi MIT: aplikasi dompet dan intinya, relay, indeks kunci publik,
layanan kurs, dan direktori data chain; kodenya ada di
[GitHub](https://github.com/orgs/mondaylabsltd/repositories). Setiap layanan bisa Anda
jalankan sendiri — lihat [panduan hosting sendiri](/id/docs/self-hosting).

## Apakah Vela sudah diaudit?

Kontrak tempat uang Anda disimpan — Safe beserta modul-modulnya, dan EntryPoint ERC-4337
— sudah diaudit. Kode Vela sendiri tidak, dan tidak ada audit yang dijadwalkan. Lihat
[audit & masalah yang diketahui](/id/docs/security-audits).

## Bagaimana kalau Vela tutup?

Dana Anda tetap berada di Safe Anda on-chain. Untuk dompet yang sudah ada, ekstensi
browser Vela dan aplikasi yang Anda kompilasi sendiri tetap berfungsi tanpa getvela.app,
dan setiap layanan open source sehingga bisa dijalankan pihak lain.
[Panduan hosting sendiri](/id/docs/self-hosting#if-getvela-app-disappears) mencantumkan
jalur-jalurnya beserta batasannya.

## Pertanyaan saya tidak ada di sini.

Buka issue di [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues), atau hubungi
kami di [X](https://x.com/realvelawallet) atau [Telegram](https://t.me/velawallet).
