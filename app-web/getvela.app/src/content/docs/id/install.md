---
title: Memasang Vela
description: "Semua cara menjalankan Vela — web, ekstensi browser, desktop, dan ponsel — berapa biaya masing-masing, apa yang bisa dilakukan, dan apa yang dibutuhkan perangkat Anda."
source: f88fdfac1001
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Memasang Vela

Dompet yang sama berjalan di beberapa tempat, dan semuanya membuka alamat yang sama
dengan kunci yang sama. Pilih sesuai kebutuhan; Anda boleh memakai lebih dari satu.
Unduhannya ada di [Dapatkan Vela](/id/get-started).

| | Apa itu | Biaya | Status |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) di browser apa pun yang cukup baru | Gratis | Sudah tersedia |
| **Ekstensi browser** | Dompet di bilah alat browser; bisa terhubung ke dApp | Gratis | Unduh lalu muat secara manual; belum ada di Chrome Web Store |
| **Desktop** | Aplikasi native untuk macOS, Windows, dan Linux | Gratis | Unduh dari Dapatkan Vela atau GitHub |
| **iPhone, Android** | Aplikasi native | Sekali beli di toko aplikasi | Belum ada di toko aplikasi; bisa Anda kompilasi dari kode sumber |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Buka dompet web →</a>

## Web

Tidak ada yang perlu dipasang. Buka [wallet.getvela.app](https://wallet.getvela.app/),
buat dompet atau masuk, dan dompet Anda sudah ada. Daftar akun Anda disimpan di browser
ini; di perangkat lain, cukup masuk lagi dengan salah satu kunci Anda.

## Ekstensi browser

Untuk browser berbasis Chromium: Chrome, Edge, dan Brave (Chrome 116 atau yang lebih
baru). Ekstensi ini menaruh dompet di bilah alat dan memungkinkan dApp terhubung
langsung dengannya. Selama belum ada di Chrome Web Store:

1. Unduh ekstensinya dari [Dapatkan Vela](/id/get-started) lalu ekstrak ke folder yang
   akan Anda simpan — browser menjalankan ekstensi dari sana.
2. Buka `chrome://extensions` dan aktifkan **Mode developer**.
3. Klik **Muat yang belum dibuka** (*Load unpacked*) dan pilih folder tersebut.

Ini dompet yang sama: ekstensi dan dompet web memakai passkey `getvela.app` yang sama,
jadi kunci yang sama membuka alamat yang sama.

## Desktop

Aplikasi native, bukan halaman web di dalam jendela: **Windows** 10 dan 11 (x64 dan
ARM), **macOS** 11 atau yang lebih baru, dan **Linux** (.deb, .rpm, atau Flatpak, x64
dan ARM).

- **Windows** akan menampilkan peringatan "Windows melindungi PC Anda", karena
  penginstalnya belum ditandatangani kodenya (code signing). Pilih **Info
  selengkapnya**, lalu **Tetap jalankan**.
- Build **macOS** ditandatangani dan dinotarisasi Apple dalam langkah terpisah,
  sehingga bisa tertinggal dari platform lain. Kalau tombol Mac bertuliskan "Segera
  tersedia", build Mac terbaru yang sudah dinotarisasi ada di halaman rilis GitHub.
- **Linux**: untuk memakai kunci keamanan USB, sistem Anda harus memberi aplikasi akses
  ke kunci itu — paket .deb dan .rpm memasang aturannya untuk Anda.

Di macOS dan Windows, aplikasi desktop punya browser bawaan untuk dApp. Checksum setiap
paket ada di [halaman rilis GitHub](https://github.com/mondaylabsltd/vela-wallet/releases).

## iPhone dan Android

Aplikasi native untuk iOS 17.4 atau yang lebih baru dan Android 10 atau yang lebih
baru. Keduanya akan dijual sebagai pembelian sekali bayar di App Store dan Google Play;
saat ini **belum ada di toko aplikasi**. Kodenya terbuka, jadi Anda bisa
mengompilasinya sendiri secara gratis — dengan satu perbedaan: build yang Anda
tandatangani sendiri tidak bisa memakai passkey bawaan ponsel Anda untuk dompet
getvela.app, meskipun memindai dengan ponsel lain dan kunci keamanan USB tetap
berfungsi. Lihat [mengompilasi aplikasi sendiri](/id/docs/self-hosting#web-app).

## Memakai Vela dengan dApp

<span id="dapps"></span>

dApp terhubung ke Vela dengan cara yang sama seperti ke dompet browser lainnya
(EIP-1193 dan EIP-6963):

- di browser desktop, lewat **ekstensi browser Vela**;
- di dalam **aplikasi desktop** (macOS, Windows), **aplikasi iPhone**, dan **aplikasi
  Android**, lewat browser bawaannya.

Dompet web di wallet.getvela.app tidak terhubung ke dApp, dan WalletConnect tidak
didukung. Setiap permintaan dari dApp didekode dan ditampilkan kepada Anda sebelum Anda
menandatangani — lihat [clear signing](/id/docs/clear-signing).

## Yang dibutuhkan perangkat Anda

Vela menandatangani dengan **passkey**, yang didukung hampir semua perangkat keluaran
beberapa tahun terakhir:

| Perangkat | Yang didukung |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16+, macOS dengan Safari atau Chrome versi terbaru |
| Android | Android versi terbaru dengan layanan Google Play, atau kunci keamanan USB |
| Windows | Windows Hello dengan Chrome atau Edge, atau kunci keamanan |
| Linux | Kunci keamanan, atau ponsel di dekat Anda (pindai kode QR) |

Kalau perangkat Anda tidak bisa menyimpan passkey sendiri, pakai ponsel lain atau kunci
keamanan fisik. [Kunci penanda tangan & kunci keamanan](/id/docs/signers) mencantumkan
jenis kunci yang didukung tiap aplikasi.

## Hanya ini alamat resminya

- **getvela.app** — situs ini, beserta unduhannya
- **wallet.getvela.app** — dompet web
- **github.com/mondaylabsltd** — kode dan paket rilis

<Callout type="warning" title="Periksa sebelum memasang">
Kalau ada yang mengarahkan Anda ke tempat lain untuk "memasang Vela" atau "memverifikasi
dompet Anda", berhentilah. Vela tidak pernah meminta frasa pemulihan — Vela memang tidak
punya frasa pemulihan.
</Callout>

Berikutnya: [membuat dompet Anda](/id/docs/create-wallet).
