---
title: Trusted Signer
description: "Halaman satu file di sign.getvela.app yang mendekode permintaan dan menandatanganinya dengan passkey Anda secara mandiri — apa yang diperiksanya, aplikasi mana yang memakainya, dan cara mem-build ulang atau menjalankan salinan Anda sendiri."
source: fcfb268d3492
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela mendekode setiap transaksi sebelum Anda menyetujuinya, dan dekode itu dikerjakan
dengan jujur — tetapi dikerjakan oleh aplikasi yang sama yang menyusun transaksinya.
Kalau aplikasi itu, atau jalur yang membawanya sampai ke Anda, dimanipulasi, aplikasi itu
bisa menampilkan satu hal dan menandatangani hal lain. Persis itulah yang terjadi pada
[Bybit](/id/docs/bybit-attack).

Trusted Signer ada untuk memisahkan keduanya: aplikasi hanya menyerahkan permintaannya,
sedangkan pemeriksaan dan tanda tangannya terjadi di halaman terpisah — halaman yang bisa
Anda baca dari awal sampai akhir, build ulang hingga sama persis byte demi byte, atau
jalankan sendiri.

## Di mana ia berjalan

Halaman resminya disajikan dari **sign.getvela.app**. Aplikasi desktop (macOS, Windows,
Linux), iPhone, dan Android bisa mengirim permintaan ke sana: aplikasi membuka halaman itu
di tab browser dengan permintaannya di dalam tautan, Anda memeriksanya lalu menandatangani
dengan passkey Anda di sana, dan halaman itu mengembalikan tanda tangannya ke aplikasi
lewat tautan `velawallet://`. Dompet web tidak bisa memakainya.

Fitur ini opsional dan harus Anda pilih sendiri. Anda memilihnya sebagai cara
menandatangani saat membuat dompet atau masuk, dan sejak itu setiap tanda tangan untuk
dompet itu di perangkat itu melewatinya. Ia juga bisa membuat kunci dompet. Di
`sign.getvela.app`, ia memakai passkey `getvela.app` yang sama dengan aplikasi.

<Callout type="info" title="Yang sudah diuji sejauh ini">
Uji menyeluruh dari awal sampai akhir terhadap halaman yang sudah dipublikasikan yang
tercatat: Android, dan Windows 11 (membuat dompet dan masuk). Aplikasi macOS, Linux, dan
iPhone memakai sambungan yang sama; belum ada satu pun yang punya catatan uji menyeluruh.
</Callout>

## Apa yang dilakukannya sebelum menandatangani

- **Mendekode permintaannya sendiri.** Apa yang dilakukan panggilan itu, kepada siapa, dan
  berapa jumlahnya, langsung dari calldata — termasuk panggilan yang bersarang di dalam
  sebuah batch.
- **Hanya menandatangani digest yang dihitungnya sendiri.** Digest EIP-191, EIP-712,
  SafeOp, dan SafeMessage dihitung di halaman itu, tidak pernah diambil dari peminta;
  pengujian mencocokkan digest SafeOp dan SafeMessage dengan `vela-core`, kode yang
  dipakai dompet, dan aplikasi menolak tanda tangan atas digest apa pun selain yang
  dihitungnya sendiri.
- **Memeriksa bahwa transaksinya memang yang diminta.** Panggilan yang diminta situs itu
  harus benar-benar ada di dalam operasi yang ditandatangani; kalau tidak, halaman itu
  menolak.
- **Mengatakan kalau sebuah persetujuan tanpa batas.** Halaman ini tidak bisa mengubah
  jumlah — ia menandatangani byte yang tiba atau tidak sama sekali — jadi persetujuan atau
  permit tanpa batas (2^200 atau lebih, 2^152 untuk Permit2 — batas yang sama dengan di aplikasi) ditampilkan merah dengan alasan itu
  dan bisa ditandatangani apa adanya; batas on-chain dipilih di layar persetujuan dompet itu
  sendiri, sebelum permintaannya sampai ke sini. Persetujuan untuk seluruh koleksi NFT
  ditolak.
- **Menolak apa yang tidak bisa dipertanggungjawabkannya:** `eth_sign`, metode yang tidak
  dikenalnya, token yang dikirim ke kontrak token itu sendiri, operasi yang tidak bisa
  dibacanya, dan proses masuk yang challenge-nya diberikan oleh peminta.
- **Menolak apa pun yang akan menyerahkan akun Anda,** dengan aturan yang sama
  seperti di aplikasi: panggilan dari akun Anda ke salah satu fungsinya sendiri
  untuk pemilik, modul, guard, atau fallback, termasuk di dalam batch;
  `delegatecall`, kecuali ke kontrak MultiSend milik Safe yang menggabungkan
  panggilan-panggilan dalam satu operasi; dan tanda tangan `SafeTx`. Halaman ini
  memeriksa setiap panggilan dalam operasi yang disusun aplikasi, bukan hanya
  panggilan yang diminta situs.
- **Menampilkan alamat akun dan identicon yang dihitung di halaman itu.** Penerima dan
  kontrak tidak pernah diberi nama berdasarkan permintaan — hanya tabel milik halaman itu
  sendiri yang sudah ditinjau yang bisa menamai sebuah kontrak. Nama akun itu sendiri, yang
  dikirim aplikasi supaya Anda bisa memilih passkey yang tepat, ditampilkan di samping
  alamatnya.
- **Meminta verifikasi pengguna** (sidik jari, wajah, atau PIN Anda) di setiap tanda
  tangan.

## Apa yang sengaja tidak dimilikinya

- **Tanpa editor.** Permintaannya sudah tetap saat tiba: Anda menandatanganinya atau
  tidak. Pemilih biaya atau editor allowance akan menulis ulang calldata, dan justru itu
  penyakit yang hendak dicegah halaman ini.
- **Tanpa akses jaringan.** Halaman ini satu file yang kebijakan keamanan kontennya
  (`default-src 'none'`) ada di dalam byte-nya sendiri, jadi ia tidak bisa mengambil apa
  pun, membuka koneksi, atau memuat gambar. Satu-satunya yang keluar darinya adalah
  jawabannya, saat ia mengikuti tautan callback di dalam permintaan (`velawallet://` kalau
  yang meminta adalah aplikasi Vela). Logo token digambar sebagai huruf.

## Apa yang diperiksa aplikasi sebagai balasannya

Aplikasi juga tidak memercayai halaman itu. Aplikasi hanya menerima tanda tangan kalau
challenge yang ditandatangani adalah digest **yang dihitung aplikasi**, verifikasi
pengguna sudah dilakukan, kuncinya adalah salah satu kunci dompet Anda, dan tanda tangan
P-256 terverifikasi dengan kunci itu.

## Setiap versi yang dipublikasikan bisa diperiksa

Setiap versi di-build dari `app-web/trusted-signer/src/` menjadi satu file, secara
reproducible — Bun dan Node menghasilkan byte yang sama — dan dipublikasikan di alamatnya
sendiri, `sign.getvela.app/b/<sha256>/sign.html`, berdampingan dengan setiap versi
sebelumnya. Daftarnya ada di `sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

Saat dijalankan, aplikasi desktop mengambil versi terpublikasi yang akan dibukanya,
menghitung hash-nya, dan membandingkannya dengan versi-versi yang tertanam di dalam
aplikasi. Hasilnya hanya dicatat di log, dan halaman yang tidak cocok tetap dibuka.
Aplikasi ponsel belum memeriksanya.

## Menjalankan salinan Anda sendiri

Pengaturan menyimpan alamat halaman yang dibuka aplikasi Anda, jadi Anda bisa
mengarahkannya ke deployment Anda sendiri: alamat HTTPS apa pun, atau `localhost` untuk
pengujian. Build dengan `bun samples/build-single.mjs` (atau `node`) lalu salin `dist/`
ke host Anda.

Salinan di domain Anda sendiri menandatangani dengan passkey yang dibuat untuk domain
**itu**, bukan dengan passkey `getvela.app` — jadi ini cara untuk membuat dan memakai
dompet yang kuncinya berada di bawah domain Anda, bukan cara untuk menandatangani bagi
dompet `getvela.app` yang sudah ada. Semua kunci sebuah dompet berbagi satu domain.

Kode, beserta skrip yang mem-build dan memeriksanya, ada di `app-web/trusted-signer/`.
