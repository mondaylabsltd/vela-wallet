---
title: Tanya jawab
description: Pertanyaan umum tentang Vela — swakelola, passkey, akun pintar, pemulihan, jaringan yang didukung, biaya, dan privasi.
---

# Tanya jawab

## Apakah Vela bersifat swakelola?

Ya. Dompet Anda adalah akun pintar yang dikendalikan oleh kunci yang hanya Anda yang
bisa memakainya, dipegang sistem operasi perangkat Anda dan tidak pernah dilihat Vela.
Vela tidak bisa memindahkan, membekukan, atau memulihkan dana Anda.

## Dompet saya akun biasa atau kontrak?

Ia **akun pintar Safe** (sebuah kontrak pintar), dijalankan dengan abstraksi akun
ERC-4337. Itulah yang memungkinkan Anda menandatangani dengan passkey, membaca setiap
transaksi sebelum menyetujuinya, dan memakai alamat yang sama di setiap jaringan. Lihat
[whitepaper](/id/docs/whitepaper) untuk arsitekturnya.

## Benar-benar tidak ada frasa pemulihan?

Benar. Kunci penanda tangan Anda adalah passkey yang dipegang sistem operasi perangkat
Anda, dan Vela tidak pernah melihatnya. Tidak ada rangkaian dua belas kata untuk
dicatat, hilang, atau di-phishing. Baca
[cara kerja passkey](/id/docs/passkeys) untuk tahu kenapa itu aman.

## Bagaimana kalau ponsel saya hilang?

Kalau passkey Anda tersinkron lewat iCloud Keychain atau Google Password Manager, Anda
masuk di perangkat baru dengan akun yang sama dan dompet Anda kembali. Lihat
[pemulihan & masuk](/id/docs/recovery) untuk model lengkap dan batasnya.

## Jaringan dan token apa saja yang didukung?

Vela hadir dengan **12 jaringan EVM** — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, dan World Chain — ditambah
jaringan buatan sendiri, menyimpan token asli dan ERC-20. Alamat Anda sama di semuanya.
Lihat [jaringan & biaya](/id/docs/networks-and-fees).

## Berapa biayanya?

Dompetnya gratis dan Vela **tidak punya token**. Anda membayar **gas** jaringan dari
saldo dompet Anda sendiri, ditambah biaya relay. Harganya dikuotasi relay dan
ditampilkan sebagai rincian _biaya jaringan / biaya relay / total_ **sebelum Anda
menandatangani** — ongkos persis setiap transaksi ada di layar konfirmasi, dan jumlah
yang dikuotasi adalah bagian dari yang Anda tandatangani, jadi ia tidak bisa berubah
sesudahnya. Transaksi yang sangat murah bisa terkena biaya minimum kecil. Di Tempo,
yang tidak punya koin asli, gas dibayar dengan stablecoin USD. Setiap jaringan juga
memerlukan **deposit kecil yang tidak dapat dikembalikan untuk mengaktifkan akun relay
gas**-nya (Vela mungkin mensponsorinya untuk pengguna baru); karena akun itu bisa
menipis, Anda mungkin harus mengisinya lagi nanti — jadi ia tidak benar-benar sekali
jalan. Detailnya di [jaringan & biaya](/id/docs/networks-and-fees).

## Apa yang bisa dilihat atau dilakukan Vela (perusahaan)?

Vela menyimpan kunci **publik** passkey Anda dan **nama** yang Anda pilih, untuk
memungkinkan masuk lintas perangkat. Ia tidak bisa melihat kunci privat Anda, saldo
Anda dibaca dari rantai publik, dan tidak ada pendaftaran lewat email.
[Kebijakan privasi](/privacy) adalah versi yang mengikat.

## Apakah Vela bersumber terbuka?

Ya — dompetnya dan empat layanan pendukungnya (data rantai, indeks passkey, relay, kurs
mata uang) [terbuka di GitHub](https://github.com/mondaylabsltd/vela-wallet) dengan
lisensi MIT, dan Anda bisa meng-hosting-nya sendiri.

## Saya punya pertanyaan yang tidak ada di sini.

Buka issue di [GitHub](https://github.com/mondaylabsltd/vela-wallet) atau hubungi kami
di [X](https://x.com/realvelawallet) atau [Telegram](https://t.me/velawallet).
