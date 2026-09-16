---
title: Serangan Bybit
description: "Pada Februari 2025 Bybit kehilangan sekitar $1,5 miliar. Kontrak Safe tidak jebol — antarmukanya yang jebol. Halaman ini menjelaskan jalur yang dipakai, dan apa dalam desain Vela yang menutupnya."
---

# Serangan Bybit, dan jalur yang dipakainya

Pada 21 Februari 2025, Bybit kehilangan kira-kira **$1,5 miliar** dari dompet dingin
multisig Safe. Itu pencurian terbesar dalam sejarah industri ini, dan layak dibaca
dengan saksama, karena hampir semua hal di dalamnya *benar* kecuali satu hal.

## Apa yang terjadi

Versi singkatnya, dari laporan-laporan publik:

1. Penyerang menguasai **mesin seorang pengembang `Safe{Wallet}`** dan menyuntikkan
   JavaScript berbahaya ke bucket AWS S3 yang melayani front-end `Safe{Wallet}`.
   Kodenya masuk pada 19 Februari dan dipicu pada 21 Februari, diarahkan ke Safe milik
   Bybit secara spesifik.
2. Para penanda tangan Bybit membuka antarmukanya dan meninjau transaksi yang tampak
   biasa saja.
3. Muatan yang sebenarnya dikirim ke **dompet perangkat keras** mereka bukan transaksi
   itu. Itu adalah `delegatecall` yang menimpa `masterCopy` proxy Safe — slot 0 —
   mengganti seluruh implementasi akun dengan milik penyerang.
4. Para penanda tangan menyetujui. Tanda tangannya sah. Kontraknya melakukan persis apa
   yang diperintahkan.

Atribusinya secara publik diarahkan ke aktivitas terkait Korea Utara (FBI menyebut
klaster TraderTraitor).

## Yang *tidak* jebol

- **Bukan kontrak Safe.** Mereka mengeksekusi instruksi yang ditandatangani secara sah.
  Tidak ada bug Safe yang dieksploitasi.
- **Bukan kriptografinya.** Setiap tanda tangan asli.
- **Bukan dompet perangkat kerasnya.** Perangkat Ledger ada di jalurnya dan tetap
  menandatangani — karena dompet perangkat keras menampilkan apa yang diberikan
  kepadanya, dan yang diberikan adalah muatan berbahaya itu. Perangkat yang tidak bisa
  menerjemahkan `delegatecall` menjadi sesuatu yang bisa dinilai manusia melindungi
  *kuncinya*, bukan *keputusannya*.

Yang jebol adalah asumsi di bawah setiap antarmuka dompet: **bahwa layar yang
menjelaskan sebuah transaksi dan bita yang sedang ditandatangani adalah hal yang
sama.**

## Kenapa ini kasus umum, bukan kejadian ganjil

Setiap tanda tangan yang pernah Anda hasilkan di dompet web bersandar pada asumsi itu.
Antarmukanya menyusun muatannya, antarmukanya menggambar ringkasannya, dan tidak ada
yang independen memeriksa bahwa keduanya cocok. Kalau kode yang menyajikan antarmuka
itu diganti — oleh pipeline build yang dikuasai, CDN yang dibajak, dependensi
berbahaya, kredensial deploy yang dicuri — ringkasannya menjadi apa pun yang diinginkan
penyerang, dan tanda tangan Anda asli.

Inilah risiko yang disasar desain penandatanganan Vela. Bukan phishing. Bukan kunci
yang bocor. **Melainkan layar tanda tangan yang berbohong kepada Anda.**

## Apa yang Vela lakukan soal itu

**Clear signing, sampai ke calldata.** Setiap transaksi diterjemahkan menjadi maksud
yang bisa dibaca manusia sebelum Anda menyetujuinya — jumlah, penerima, apa sebenarnya
yang dilakukan panggilan itu ([ERC-7730](/id/docs/clear-signing)). Panggilan yang tidak
bisa kami terjemahkan **ditandai sebagai tak terbaca**, bukan digambar diam-diam
seolah-olah baik-baik saja. Muatan Bybit adalah `delegatecall` yang menukar alamat
implementasi; itu persis bentuk hal yang seharusnya membuat penanda tangan berhenti
seketika, dan menyembunyikannya di balik ringkasan ramah adalah alasan kenapa itu tidak
terjadi.

**Jalur independen yang bisa memeriksa antarmukanya.** Vela sedang membangun halaman
tanda tangan tanpa build dan tanpa dependensi yang menggambar maksudnya sendiri dan
melakukan tanda tangan WebAuthn-nya sendiri — satu folder berkas statis yang bisa Anda
baca dari ujung ke ujung, Anda sajikan sendiri, atau Anda jalankan sebagai ekstensi
browser. Seluruh tujuannya adalah menjadi pendapat kedua yang tidak berbagi rantai
pasok dengan aplikasi utama. *Status: sudah dibuat dan diuji, belum dirilis.* Saat
dirilis sifatnya opsional, dan halaman ini akan menyatakannya terang-terangan ketika
itu berubah.

**Tidak ada kontrak yang bisa kami perbarui.** Muatan Bybit bekerja dengan mengganti
implementasi akunnya. Akun Vela adalah
[Safe v1.4.1 tanpa modifikasi](/id/docs/account-contract) dan Vela tidak memegang peran
istimewa apa pun padanya — tidak ada kunci admin, tidak ada jalur peningkatan yang bisa
dipaksakan atau disusupi untuk kami pakai.

**Biometrik baru untuk setiap tanda tangan.** Tidak ada kunci sesi berumur panjang,
jadi tidak ada jendela waktu di mana sesuatu bisa menandatangani atas nama Anda tanpa
Anda hadir.

**Hosting sendiri sebagai jaring pengaman.** Aplikasi dan setiap layanan pendukungnya
bersumber terbuka. Kalau Anda sama sekali tidak ingin memercayai pipeline build kami,
jalankan sendiri — itu satu-satunya jawaban untuk kelas serangan ini yang tidak
menuntut Anda memercayai siapa pun.

## Yang tidak Vela klaim

Front-end Vela bisa saja dikuasai dengan cara yang sama seperti `Safe{Wallet}`. Kode
kami belum diaudit. Mengatakan sebaliknya justru jenis jaminan yang seharusnya
diakhiri oleh insiden ini.

Yang coba dilakukan desainnya adalah mempersempit jalurnya: membuat muatannya terbaca
alih-alih buram, menghapus primitif peningkatan yang diandalkan serangan itu, dan
memberi Anda cara memverifikasi dengan sesuatu yang bukan kami. Ringkasan jujurnya:
**kelas serangan ini diperkecil oleh desain, bukan dihilangkan**, dan bagian-bagian
yang akan mengeraskannya lebih jauh tercantum, belum selesai, di
[audit & masalah yang diketahui](/id/docs/security-audits).

## Sumber

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
