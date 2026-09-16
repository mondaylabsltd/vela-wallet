---
title: Kirim & terima
description: Cara menerima dan mengirim token di Vela — satu alamat untuk semua jaringan, transaksi yang ditandatangani secara jelas, dan bagaimana abstraksi akun benar-benar memindahkan dana Anda.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Kirim & terima

## Menerima

1. Buka dompet Anda dan ketuk **Terima**.
2. Bagikan alamat Anda — salin, atau biarkan pengirim memindai kode QR.
3. Saat transfer terkonfirmasi di rantai, saldonya muncul di dompet Anda.

Dua hal yang perlu diketahui:

- Alamat Anda **sama di setiap jaringan yang didukung**, jadi Anda membagikan satu
  alamat ke mana pun — pastikan saja pengirimnya memakai jaringan yang benar.
- Anda **bisa menerima sebelum dompet Anda dipasang**. Akun Vela adalah akun pintar
  kontrafaktual, jadi dana bisa tiba di alamat Anda sebelum kontraknya ada di suatu
  rantai; kontraknya memasang diri saat Anda pertama kali mengirim di sana.

## Mengirim

1. Ketuk **Kirim** dan pilih **token**-nya.
2. Masukkan **jumlahnya** (Anda bisa berpindah antara token dan mata uang tampilan
   Anda) dan **penerimanya**. Vela menerjemahkan penerima yang dikenalnya menjadi nama
   bila bisa — akun Vela, nama ENS, Basename, dan seterusnya.
3. **Tinjau dan konfirmasi.** Vela menampilkan transfernya, lalu meminta passkey Anda
   (Face ID / Touch ID / sidik jari).

### Apa yang terjadi saat Anda mengonfirmasi

Vela tidak sekadar "menyiarkan" transaksi. Di baliknya:

1. Ia menyusun **UserOperation** ERC-4337 untuk akun Safe Anda.
2. Perangkat Anda menandatanganinya dengan pernyataan **WebAuthn (P-256)** setelah
   pemeriksaan biometrik.
3. Operasi yang sudah ditandatangani dikirim ke **relay**, yang meneruskannya ke
   EntryPoint; Safe Anda memverifikasi tanda tangan P-256 **di rantai** lalu
   mengeksekusinya.

<Callout type="info" title="Relay tidak bisa mengutak-atik transaksi Anda">
Relay menerima UserOperation yang <strong>sudah ditandatangani</strong>. Ia bisa
menunda atau menolak meneruskan, tetapi tidak bisa mengubah penerima, jumlah, atau
kolom apa pun — perubahan apa pun membatalkan tanda tangan Anda. Ia penolong untuk
ketersediaan layanan, bukan kustodian, dan karena bersumber terbuka Anda bisa
menjalankan sendiri.
</Callout>

### Clear signing — tanpa persetujuan buta

Sebelum Anda menandatangani, Vela menerjemahkan transaksi memakai deskriptor
**ERC-7730** dan menampilkan **maksudnya** (Kirim, Setujui, Tukar…), **jumlah dan
alamatnya**, serta indikasi risiko — bukan hex yang tak terbaca. Kalau sebuah panggilan
tidak bisa diterjemahkan sepenuhnya, ia menampilkan **peringatan tanda tangan buta**
yang terang-terangan alih-alih berpura-pura paham. Persetujuan token tanpa batas tidak
sekadar ditandai — Vela menulis ulangnya menjadi jumlah terbatas dan menolak
mengirimkan persetujuan yang masih tanpa batas.

## Sebelum menekan kirim

- **Periksa karakter awal dan akhir alamatnya.** Malware penukar alamat itu nyata.
- **Pastikan jaringannya.** Mengirim di jaringan yang salah adalah kesalahan mahal
  yang paling sering terjadi. Lihat [jaringan & biaya](/id/docs/networks-and-fees).
- **Mulai kecil dengan penerima baru.** Satu transfer uji kecil adalah asuransi murah.

Transaksi tidak bisa dibatalkan. Tidak ada meja bantuan yang bisa menarik kembali
kiriman ke alamat yang salah — begitulah sifat swakelola.

## Membaca riwayat Anda

Saldo dan riwayat dibaca langsung dari sekumpulan endpoint RPC publik dengan
pengalihan otomatis. Kalau jaringannya lambat, riwayat bisa perlu waktu sebentar —
ikon berputar berarti "masih mengambil data", bukan "dana hilang".
