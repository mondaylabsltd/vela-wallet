---
title: Hosting sendiri halaman tanda tangan
description: Halaman tanpa dependensi dan ekstensi Chrome yang menerjemahkan sendiri sebuah transaksi lalu menandatanganinya dengan passkey Anda — cara menjalankan salinan Anda sendiri, dan salinan mana yang bisa menandatangani untuk dompet Anda.
---

# Hosting sendiri halaman tanda tangan

Vela menerjemahkan setiap transaksi sebelum Anda menyetujuinya, dan terjemahan itu
kerja yang jujur — tetapi kerja yang dilakukan oleh aplikasi yang sama yang menyusun
transaksinya. Kalau aplikasinya, atau jalan yang dipakainya untuk sampai ke Anda,
diutak-atik, ia bisa menampilkan satu hal dan menandatangani hal lain. Itulah persis
yang terjadi pada [Bybit](/id/docs/bybit-attack).

Halaman tanda tangan ada untuk membelah itu jadi dua: transaksinya datang dari satu
tempat, sedangkan pemeriksaan dan tanda tangannya terjadi di tempat yang Anda
kendalikan.

## Apa itu

Satu folder — `app-web/clearsigning` di repositori — yang sekaligus halaman web dan
ekstensi Chrome. HTML, CSS, dan JavaScript murni: tanpa framework, tanpa bundler,
tanpa langkah build, tanpa dependensi, dan tanpa permintaan jaringan sendiri.

Saat menerima permintaan tanda tangan, ia tidak memercayai ringkasan yang datang
bersamanya. Ia menerjemahkan sendiri calldata mentahnya, menghitung digest-nya
sendiri, menunjukkan apa yang sebenarnya akan diizinkan oleh tanda tangan itu, dan
baru setelah itu meminta passkey Anda.

Karena tidak ada langkah build, berkas yang Anda baca adalah berkas yang berjalan.
Anda bisa membandingkan folder itu dengan repositori dan tahu persis apa yang Anda
sajikan.

## Salinan mana yang bisa menandatangani untuk dompet Anda

Passkey terikat pada domain tempat ia dibuat. Kunci Vela Anda terdaftar di bawah
`getvela.app`, dan browser hanya akan menawarkannya kepada halaman yang relying
party-nya `getvela.app`. Satu aturan itu saja yang menentukan cara menjalankan salinan
mana yang berguna bagi Anda.

**Sebagai ekstensi Chrome — inilah yang dipakai dengan dompet Anda yang sudah ada.**
Relying party ekstensi itu adalah `getvela.app` dari mana pun foldernya berasal, jadi
kunci Anda yang sekarang bisa menandatangani di dalamnya, sementara kodenya adalah
folder yang Anda muat dan Anda periksa sendiri.

1. Buka `chrome://extensions` dan nyalakan **Mode pengembang**.
2. **Muat yang belum dipaket**, lalu pilih folder `app-web/clearsigning`.
3. Ikon di bilah alat membuka halamannya di sebuah tab.

**Sebagai halaman di domain Anda sendiri, atau di localhost.** Saat disajikan lewat
HTTP(S), relying party halaman itu adalah nama host-nya sendiri — jadi ia bisa
menandatangani dengan kunci yang terdaftar di bawah _nama host itu_, bukan kunci yang
terdaftar di bawah `getvela.app`. Itu membuatnya cara yang tepat untuk mencoba seluruh
prosesnya dari ujung ke ujung, menjalankan alur desktop, dan menandatangani untuk
dompet yang kuncinya dibuat di domain Anda sendiri. Ia bukan cara untuk menandatangani
bagi dompet `getvela.app` yang sudah ada.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Setiap jalur di dalam aplikasinya bersifat relatif, jadi subdirektori di host yang
sudah ada pun bisa; membuka `index.html` langsung dari disk (`file://`) cukup untuk
melihat-lihat — tanpa origin tidak ada relying party dan tidak ada yang bisa
ditandatangani.

## Apa yang dilakukannya sebelum menandatangani

- **Ia menerjemahkan sendiri transaksinya.** Apa yang dilakukan panggilan itu, kepada
  siapa, dan berapa banyak, dari calldata-nya — termasuk panggilan yang bersarang di
  dalam sebuah batch.
- **Ia hanya menandatangani digest yang dihitungnya sendiri.** Digest EIP-191,
  EIP-712, SafeOp, dan SafeMessage dihitung di dalam halaman dan dicocokkan silang
  dengan `vela-core`, kode yang sama yang dipakai dompetnya. Digest yang tidak bisa
  dihitungnya berarti penolakan, bukan tanda tangan.
- **Ia memastikan transaksinya memang yang diminta.** Panggilan yang diminta situsnya
  harus benar-benar ada di dalam operasi yang sedang ditandatangani.
- **Ia menolak persetujuan tanpa batas.** Bukan peringatan — penolakan, lengkap dengan
  petunjuk apa yang sebaiknya dilakukan.
- **Ia mengatakan kalau ada yang tidak bisa dibacanya,** alih-alih menyajikan ringkasan
  ramah yang tidak bisa dipertanggungjawabkannya.
- **Ia menampilkan alamat dan identicon akunnya,** dan tidak menampilkan nama penerima
  yang diberikan pihak yang meminta tanda tangan. Apa pun yang dikendalikan si peminta
  dibuang atau diberi label sebagai miliknya.

## Yang sengaja tidak dimilikinya

- **Tidak ada kolom edit.** Permintaannya terkunci begitu tiba: Anda menandatanganinya
  atau tidak. Pemilih biaya atau penyunting jatah akan menulis ulang calldata, dan itu
  justru penyakit yang ingin dicegah halaman ini.
- **Tidak ada pembuatan kunci.** Halaman tanda tangan tidak bisa membuat passkey.
  Membuatnya sama dengan membuat akun yang lain.
- **Tidak ada permintaan jaringan.** Kalau tidak ada yang diambil, tidak ada yang bisa
  disadap.

## Bagaimana sebuah permintaan sampai kepadanya

| Peminta | Kanal |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Halaman di browser yang sama | `postMessage` |
| Halaman di browser yang sama, ke ekstensinya | Port ekstensi |
| Aplikasi desktop di mesin yang sama | Fragmen URL + callback loopback |
| Ponsel atau komputer lain | Bluetooth LE (protokolnya sudah dibuat; sisi radionya belum diuji di perangkat nyata) |

Format datanya, digest-nya, dan tabel asal setiap elemen di layar ada di `PROTOCOL.md`
di samping kodenya.

## Kapan memakainya

Sejak hari akun itu menyimpan uang yang akan Anda sayangkan kalau hilang — dan sejak
itu, untuk setiap tanda tangan. Bukan hanya untuk jumlah besar: satu persetujuan kecil
bisa menyerahkan cukup wewenang untuk mengosongkan akun. Kebiasaan menandatangani yang
hanya dipakai pada momen istimewa tidak akan ada di tempatnya pada hari ia dibutuhkan.
