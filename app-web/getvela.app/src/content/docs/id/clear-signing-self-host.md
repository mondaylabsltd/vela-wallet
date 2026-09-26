---
title: Hosting sendiri halaman tanda tangan
description: "Halaman dan ekstensi Chrome tanpa dependensi yang mendekode transaksi sendiri lalu menandatanganinya dengan passkey Anda — cara menjalankan salinan Anda sendiri, dan salinan mana yang bisa menandatangani untuk dompet Anda."
source: c81389ef7aa2
---

# Hosting sendiri halaman tanda tangan

Vela mendekode setiap transaksi sebelum Anda menyetujuinya, dan dekode itu dikerjakan
dengan jujur — tetapi dikerjakan oleh aplikasi yang sama yang menyusun transaksinya.
Kalau aplikasi itu, atau jalur yang membawanya sampai ke Anda, dimanipulasi, aplikasi itu
bisa menampilkan satu hal dan menandatangani hal lain. Persis itulah yang terjadi pada
[Bybit](/id/docs/bybit-attack).

Halaman tanda tangan ada untuk memisahkan keduanya: transaksinya datang dari satu tempat,
sedangkan pemeriksaan dan tanda tangannya terjadi di tempat yang Anda kendalikan.


Saat menerima permintaan tanda tangan, halaman ini tidak memercayai ringkasan yang ikut
datang bersamanya. Halaman ini mendekode calldata mentah sendiri, menghitung digest-nya
sendiri, menunjukkan kepada Anda apa yang benar-benar akan diotorisasi tanda tangan itu,
dan baru setelah itu meminta passkey Anda.

Karena tidak ada langkah build, file yang Anda baca adalah file yang dijalankan. Anda bisa
membandingkan (diff) folder itu dengan repositorinya dan tahu persis apa yang Anda
sajikan.

## Salinan mana yang bisa menandatangani untuk dompet Anda

Sebuah passkey terikat pada domain tempat passkey itu dibuat. Kunci Vela Anda terdaftar
di bawah `getvela.app`, dan browser hanya akan menawarkannya ke halaman yang relying
party-nya adalah `getvela.app`. Satu aturan itulah yang menentukan cara menjalankan
salinan sendiri mana yang berguna bagi Anda.

**Sebagai halaman di domain Anda sendiri, atau di localhost.** Kalau disajikan lewat
HTTPS (atau dari localhost), relying party halaman itu adalah nama host-nya sendiri —
jadi halaman itu bisa menandatangani dengan kunci yang terdaftar di bawah nama host
_tersebut_, bukan dengan kunci yang terdaftar di bawah `getvela.app`. Karena itu, cara
ini cocok untuk mencoba seluruh prosesnya dari awal sampai akhir, menjalankan alur
desktop, dan menandatangani untuk dompet yang kuncinya dibuat di domain Anda sendiri.
Cara ini bukan cara untuk menandatangani bagi dompet `getvela.app` yang sudah ada.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Semua path di aplikasi ini relatif, jadi subdirektori di host yang sudah ada juga bisa
dipakai, dan membuka `index.html` langsung dari disk (`file://`) bisa untuk melihat-lihat
— tanpa origin, tidak ada relying party dan tidak ada yang bisa ditandatangani.

## Apa yang dilakukannya sebelum menandatangani

- **Mendekode transaksinya sendiri.** Apa yang dilakukan panggilan itu, kepada siapa, dan
  berapa jumlahnya, langsung dari calldata — termasuk panggilan yang bersarang di dalam
  sebuah batch.
- **Hanya menandatangani digest yang dihitungnya sendiri.** Digest EIP-191, EIP-712,
  SafeOp, dan SafeMessage dihitung di halaman itu dan dicocokkan dengan `vela-core`, kode
  yang sama dengan yang dipakai dompet. Digest yang tidak bisa dihitungnya berarti
  penolakan, bukan tanda tangan.
- **Memeriksa bahwa transaksinya memang yang diminta.** Panggilan yang diminta situs itu
  harus benar-benar ada di dalam operasi yang ditandatangani.
- **Mengatakan kalau sebuah persetujuan tanpa batas.** Halaman ini tidak bisa mengubah
  jumlah — ia menandatangani byte yang tiba atau tidak sama sekali — jadi persetujuan atau
  permit tanpa batas (2^128 atau lebih di halaman ini) ditampilkan merah dengan alasan itu
  dan bisa ditandatangani apa adanya; batas on-chain dipilih di layar persetujuan dompet itu
  sendiri, sebelum permintaannya sampai ke sini. Persetujuan untuk seluruh koleksi NFT
  ditolak.
- **Mengatakan kalau tidak bisa membaca sesuatu,** alih-alih menampilkan ringkasan ramah
  yang tidak bisa dipertanggungjawabkannya.
- **Menampilkan alamat dan identicon akun,** dan tidak menampilkan nama penerima yang
  diberikan oleh pihak yang meminta tanda tangan. Apa pun yang dikendalikan peminta akan
  dibuang atau diberi label sebagai milik peminta.

## Apa yang sengaja tidak dimilikinya

- **Tanpa editor.** Permintaannya sudah tetap saat tiba: Anda menandatanganinya atau
  tidak. Pemilih biaya atau editor allowance akan menulis ulang calldata, dan justru itu
  penyakit yang hendak dicegah halaman ini.
- **Tanpa pembuatan kunci.** Halaman tanda tangan tidak bisa membuat passkey. Membuat
  passkey berarti membuat akun yang berbeda.
- **Tanpa data dari jaringan.** Tidak ada yang ditampilkan atau ditandatanganinya yang
  diambil dari luar. Satu-satunya yang dimuatnya adalah logo token, sebagai gambar, dari
  server data chain milik Vela; kalau gagal, sebuah huruf menggantikannya.

## Bagaimana permintaan sampai ke sana

| Peminta                                      | Saluran                                                                    |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Halaman di browser yang sama                 | `postMessage`                                                              |
| Halaman di browser yang sama, ke ekstensi    | Port ekstensi                                                              |
| Aplikasi desktop di komputer yang sama       | Fragmen URL + callback loopback (demo di `samples/`; aplikasi desktop Vela belum memakainya) |
| Ponsel atau komputer lain                    | Bluetooth LE (protokolnya sudah diimplementasikan; radionya belum diuji di perangkat keras sungguhan) |

Format data, digest, dan tabel asal setiap item di layar ada di `PROTOCOL.md` di samping
kodenya.

## Posisinya

Begitu aplikasi bisa menyerahkan permintaannya ke halaman ini, cara pakai yang dituju
sederhana: sejak hari akun Anda menyimpan uang yang tidak rela Anda hilangkan, setiap
tanda tangan melewati halaman yang kodenya Anda muat sendiri. Bukan hanya untuk jumlah
besar — persetujuan kecil pun bisa menyerahkan cukup banyak untuk mengosongkan akun.
Sampai saat itu, halaman ini adalah cara untuk membaca dan menguji persis bagaimana
pendapat kedua itu akan bekerja.
