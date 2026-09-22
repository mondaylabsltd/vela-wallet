---
title: Kirim & terima
description: "Cara menerima dan mengirim dengan Vela — satu alamat di semua jaringan, mengirim ke satu atau banyak orang, dari mana nama penerima berasal, apa yang Anda konfirmasi, dan bagaimana relay memindahkan dana Anda."
source: c23b205bcd8b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Kirim & terima

## Menerima

1. Buka dompet Anda dan ketuk **Terima**.
2. Bagikan alamat Anda — salin, atau tampilkan kode QR-nya. Dompet web juga bisa
   membuat permintaan pembayaran yang mencantumkan jumlah.
3. Begitu transfer terkonfirmasi on-chain, dana itu muncul di saldo Anda.

- Alamat Anda **sama di setiap jaringan**, jadi Anda cukup membagikan satu alamat —
  tetapi pengirim tetap harus memakai jaringan yang didukung Vela, atau jaringan yang
  sudah Anda tambahkan.
- Anda **bisa menerima sebelum dompet Anda di-deploy** di suatu jaringan. Dompet itu
  men-deploy dirinya sendiri saat Anda pertama kali mengirim dari sana.

## Mengirim

1. Ketuk **Kirim** dan pilih **token**-nya.
2. Masukkan **jumlah** (dalam token atau dalam mata uang tampilan Anda) dan **alamat
   penerima**, dengan menempel, memindai kode QR, atau memilih kontak.
3. **Tinjau.** Vela menampilkan apa yang akan terjadi, biayanya, dan nama yang
   ditemukannya untuk penerima, jika ada.
4. **Konfirmasi** dengan salah satu kunci Anda — Face ID, sidik jari, PIN, atau sentuhan
   dan PIN di kunci keamanan Anda.

### Mengirim ke banyak orang, atau mengumpulkan

- **Bagi (split)** — kirim satu token ke beberapa orang dalam satu transaksi. Anda bisa
  menempel daftar atau mengimpor spreadsheet, dan memasukkan jumlah dalam mata uang
  Anda.
- **Kumpulkan (sweep)** — kirim beberapa token ke satu alamat dalam satu transaksi.

Dengan cara mana pun, Anda cukup menandatangani sekali, dan transaksinya membayar satu
kali biaya.

### Nama untuk alamat

Saat Anda memasukkan alamat, Vela mencari namanya: pertama di registrinya sendiri (nama
dompet Vela lain), lalu di catatan balik (reverse record) `.bnb`, `.arb`, `.g`,
Basename, dan ENS, yang dibaca langsung dari tiap chain. Pencarian ini satu arah — ia
memberi nama pada alamat yang Anda masukkan. Mengetik nama seperti `alice.eth` tidak
akan mencari alamatnya. **Kontak** yang Anda simpan juga menampilkan namanya. Anggap
nama sebagai petunjuk, bukan bukti: catatan balik atau nama dompet Vela dipilih oleh
siapa pun yang mengendalikan alamat itu.

### Koin biaya dan kecepatan

Layar konfirmasi menampilkan biaya dalam koin biaya dan dalam mata uang Anda. Anda bisa
membayar dengan koin jaringan itu atau, di tempat relay menerimanya, dengan stablecoin
USD, dan memilih kecepatan (bawaan: cepat). Saat Anda mengirim koin native dalam jumlah
**maksimum**, Vela menyisihkan cukup untuk biayanya.
[Cara biaya dihitung](/id/docs/networks-and-fees).

### Yang terjadi saat Anda mengonfirmasi

1. Vela menyusun **UserOperation** ERC-4337 untuk Safe Anda, termasuk pembayaran biaya
   ke relay.
2. Kunci Anda menandatanganinya dengan asersi **WebAuthn (P-256)** setelah memverifikasi
   Anda.
3. Operasi yang sudah ditandatangani dikirim ke **relay**, yang meneruskannya ke
   EntryPoint; Safe Anda memeriksa tanda tangan P-256 on-chain lalu mengeksekusinya.

<Callout type="info" title="Relay tidak bisa mengubah transaksi Anda">
Relay menerima operasi yang sudah ditandatangani. Relay tidak bisa mengubah penerima,
jumlah, atau biayanya — perubahan apa pun membuat tanda tangan Anda tidak sah. Relay bisa
menunda atau menolaknya, dan menentukan kapan transaksi itu masuk ke chain. Relay ini
open source, dan Anda bisa [menjalankannya sendiri](/id/docs/self-hosting#relay).
</Callout>

Sebelum Anda menandatangani, Vela mendekode apa yang dilakukan transaksi itu dan memberi
peringatan tentang bagian yang tidak bisa didekodenya; lihat
[clear signing](/id/docs/clear-signing).

## Sebelum menekan kirim

- **Periksa awal dan akhir alamatnya.** Malware penukar alamat itu nyata, begitu pula
  alamat mirip yang sengaja diselipkan ke riwayat Anda.
- **Pastikan jaringannya.** Mengirim di jaringan yang salah adalah kesalahan yang sering
  terjadi dan mahal.
- **Mulai dari jumlah kecil untuk penerima baru.** Transfer uji yang kecil adalah
  asuransi yang murah.

Transaksi tidak bisa dibatalkan. Tidak ada yang bisa menarik kembali kiriman ke alamat
yang salah — begitulah sifat dompet non-kustodial.

## Aktivitas Anda

Aktivitas Anda menggabungkan apa yang Anda kirim dari perangkat ini dengan transfer
token yang dibaca dari log tiap chain. Transfer koin native biasa yang masuk ke Anda
lewat kontrak lain (misalnya sebagian penarikan dari bursa) di beberapa jaringan
mungkin tidak menghasilkan log, sehingga bisa muncul di saldo tanpa muncul di aktivitas.
Saldo dibaca langsung lewat sekumpulan endpoint RPC dengan pengalihan otomatis; ikon
berputar berarti "masih mengambil data", bukan "dana hilang".

Berikutnya: [jaringan & biaya](/id/docs/networks-and-fees).
