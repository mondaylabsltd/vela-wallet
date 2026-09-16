---
title: Membuat dompet Anda
description: Buat dompet Vela swakelola dalam sekitar satu menit dengan passkey — tanpa frasa pemulihan. Dompet Anda adalah akun pintar Safe dengan alamat yang sama di setiap jaringan.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Membuat dompet Anda

Membuat dompet memakan waktu sekitar satu menit dan satu kali verifikasi biometrik.
Buka dompet web di [wallet.getvela.app](https://wallet.getvela.app/) lalu pilih
**Buat dompet**.

## Langkah-langkahnya

1. **Beri nama dompet Anda.** Pilih nama agar Anda mengenali akunnya nanti, termasuk
   saat masuk di perangkat lain. Nama itu disimpan di samping kunci publik Anda, jadi
   anggap saja publik — jangan menaruh sesuatu yang pribadi di situ.
2. **Setujui hal-hal dasarnya.** Daftar singkat memastikan Anda paham bahwa Vela
   bersifat swakelola dan masih perangkat lunak alfa, lengkap dengan tautan ke
   [kebijakan privasi](/privacy) dan [ketentuan](/terms).
3. **Buat passkey Anda.** Saat diminta, verifikasi dengan **Face ID, Touch ID, atau
   sidik jari**. Langkah ini membuat passkey WebAuthn (P-256) yang dipegang perangkat
   Anda dan tidak pernah dilihat Vela. Tidak ada langkah frasa pemulihan, karena
   memang tidak ada frasa pemulihan.
4. **Selesai.** Vela menampilkan alamat dompet Anda dan Anda sudah masuk. Anda bisa
   memeriksanya lalu masuk untuk sampai ke dompet Anda.

## Dompet Anda sebenarnya apa

Ini bagian yang jarang dijelaskan dompet lain — dan penting untuk memahami cara kerja
Vela.

Dompet Vela Anda adalah **akun pintar Safe** (sebuah kontrak pintar), bukan "akun milik
kunci eksternal" biasa. Passkey Anda adalah pemilik akun itu; penyiapan ERC-4337
memungkinkan Anda menjalankannya hanya dengan wajah atau sidik jari.

<Callout type="info" title="Alamat Anda sama di setiap jaringan">
Vela menurunkan alamat Anda dari kunci publik passkey, jadi alamatnya identik di
Ethereum, Base, Arbitrum, Gnosis, dan setiap jaringan lain yang didukung. Anda cukup
membagikan satu alamat ke mana pun.
</Callout>

Konsekuensi yang berguna: alamatnya bersifat **kontrafaktual**. Ia dihitung sebelum
apa pun dipasang di rantai, jadi **Anda bisa menerima dana di situ sebelum kontrak
dompet Anda ada**. Kontraknya memasang dirinya sendiri — dibayar dari saldonya sendiri
— saat pertama kali Anda mengirim transaksi di suatu jaringan.

## Apa yang barusan terjadi pada kunci Anda

- Perangkat Anda membuat **sepasang kunci passkey**.
- **Kunci privatnya** dipegang penyedia passkey sistem operasi Anda (iCloud Keychain
  atau Google Password Manager), disimpan terenkripsi ujung-ke-ujung dan disinkronkan
  antar perangkat Anda — tidak ada aplikasi, termasuk Vela, yang melihatnya.
- **Kunci publik dan nama pilihan Anda** dikirim ke Indeks Passkey milik Vela, yang
  juga menulis kunci itu ke catatan yang bisa dibaca publik di Gnosis Chain, agar akun
  Anda bisa ditemukan lagi di perangkat baru. Lihat
  [pemulihan & masuk](/id/docs/recovery).

## Langkah selanjutnya

- [Menerima token pertama Anda](/id/docs/send-and-receive)
- [Memahami jaringan dan biaya](/id/docs/networks-and-fees)
- [Membaca bagaimana passkey menjaga semuanya tetap aman](/id/docs/passkeys)
