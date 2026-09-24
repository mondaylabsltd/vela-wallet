---
title: Membuat dompet Anda
description: "Membuat dompet Vela dengan satu sampai tujuh kunci — apa yang dilakukan tiap langkah, kenapa kunci ditetapkan saat pembuatan, apa yang menjadi publik, dan dompet Anda sebenarnya apa."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Membuat dompet Anda

Membuat dompet butuh satu atau dua menit. Buka dompet web di
[wallet.getvela.app](https://wallet.getvela.app/) — atau ekstensi, aplikasi desktop,
atau aplikasi ponsel — lalu pilih **Buat Dompet**.

## Langkah-langkahnya

1. **Beri nama dompet Anda.** Nama membantu Anda mengenali dompet, dan nama itu ditulis
   ke registri publik bersama kunci Anda — anggap saja publik, dan jangan isi dengan hal
   pribadi.
2. **Pahami apa yang akan terjadi.** Anda mencentang bahwa kunci publik dan nama dompet
   Anda ditulis on-chain, bahwa kunci privat Anda tetap berada di perangkat atau kunci
   keamanan Anda, dan bahwa Anda menyetujui [Ketentuan Layanan](/terms) dan
   [Kebijakan Privasi](/privacy).
3. **Buat kunci pertama Anda.** Pilih caranya: **Perangkat ini** (Face ID, Touch ID,
   sidik jari, Windows Hello), **Ponsel atau tablet** (pindai kode QR lalu buat
   kuncinya di perangkat itu, jika aplikasinya menyediakan opsi ini), atau **Kunci
   keamanan USB**. Perangkat Anda membuat passkey lalu menandatangani sekali dengannya,
   supaya aplikasi tahu kunci itu benar-benar berfungsi sebelum melanjutkan.
4. **Tambahkan kunci lain, kalau mau.** Total hingga tujuh, dari jenis apa pun. Salah
   satunya saja nanti sudah bisa menandatangani. Kalau satu-satunya kunci Anda tidak
   tersinkron ke mana pun — kunci keamanan, atau Windows Hello — aplikasi meminta kunci
   kedua, karena satu kunci yang tidak tersinkron berarti satu perangkat hilang sudah
   cukup untuk kehilangan dompet.
5. **Buat.** Aplikasi menghitung alamat dompet Anda dari seluruh kumpulan kunci lalu
   memublikasikan kumpulan itu ke registri publik di Gnosis Chain. Begitu catatan itu
   masuk on-chain, dompet Anda terbuka.

<Callout type="warning" title="Pilih kunci Anda sekarang">
Alamat Anda dihitung dari kunci yang Anda tetapkan di akhir, jadi kunci tidak bisa
ditambah, dihapus, atau diganti belakangan. [Kunci penanda tangan & kunci keamanan](/id/docs/signers)
menjelaskan alasannya, dan cara memilihnya.
</Callout>

## Dompet Anda sebenarnya apa

Dompet Anda adalah **akun pintar Safe** — sebuah kontrak, bukan akun biasa dengan satu
kunci privat. Kunci-kunci Anda adalah pemiliknya, dan salah satunya saja bisa
mengotorisasi transaksi. [Kontrak akun](/id/docs/account-contract) mencantumkan setiap
kontrak yang terlibat.

Alamatnya **sama di setiap jaringan**, dan bersifat **kontrafaktual**: alamat itu
dihitung sebelum apa pun di-deploy, jadi Anda bisa langsung menerima dana di jaringan
mana pun. Kontraknya men-deploy dirinya sendiri saat pertama kali Anda mengirim dari
suatu jaringan, dan biaya transaksi pertama itu sudah termasuk biaya deploy. Membuat
dompet tidak memakan biaya sepeser pun.

## Apa yang publik

<span id="what-is-public"></span>

Membuat dompet menulis catatan permanen ke kontrak registri publik di Gnosis Chain —
bisa dibaca siapa pun, dan tidak bisa diubah atau dihapus:

- **kunci publik** tiap kunci (tidak pernah kunci privatnya) beserta **ID
  kredensial**-nya;
- **model autentikator** tiap kunci (pengelola kata sandi atau kunci keamanan mana yang
  membuatnya), serta penanda apakah Anda terverifikasi dan apakah kunci itu tersinkron;
- **nama dompet** dan **label untuk tiap kunci**;
- **alamat dompet** dan waktu pembuatannya;
- **data pendaftaran yang ditandatangani** itu sendiri.

Indeks kunci publik milik Vela yang mengirim catatan itu dan membayar gasnya, jadi indeks
itulah yang pertama kali melihat isinya. Tidak ada isi catatan itu yang bisa memindahkan
dana Anda; catatan inilah yang memungkinkan salah satu kunci Anda menemukan dompetnya
lagi di perangkat baru ([pemulihan](/id/docs/recovery)). Daftar lengkapnya ada di
[kebijakan privasi](/privacy), dan [halaman registri](/registry) menampilkan setiap
catatan.

## Langkah berikutnya

- [Menerima token pertama Anda](/id/docs/send-and-receive)
- [Memahami jaringan dan biaya](/id/docs/networks-and-fees)
- [Yang harus dilakukan kalau perangkat hilang](/id/docs/recovery)
