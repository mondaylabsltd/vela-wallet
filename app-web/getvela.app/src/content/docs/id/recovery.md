---
title: Pemulihan & masuk
description: "Bagaimana Anda kembali ke dompet di perangkat baru dengan salah satu kunci Anda, di mana dompet itu dicari, dan batas nyata pemulihan tanpa frasa pemulihan."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Pemulihan & masuk

Tanpa frasa pemulihan, pemulihan bertumpu pada dua hal: **kunci yang masih Anda
pegang**, dan **catatan publik tentang kunci mana saja yang menjadi milik dompet Anda**.

## Apa yang dicatat saat Anda membuat dompet

Alamat dompet Anda dihitung dari semua kunci yang Anda pakai saat membuatnya. Supaya
salah satu kunci itu bisa menemukan dompetnya lagi nanti, pembuatan dompet menulis
catatan ke **kontrak registri** publik di Gnosis Chain: kunci publik tiap kunci, alamat
dompet, namanya, dan data pendaftaran yang ditandatangani. Registri itu tidak punya
pemilik, dan catatannya tidak bisa diubah atau dihapus. (Daftar lengkap apa saja yang
publik ada di [membuat dompet Anda](/id/docs/create-wallet#what-is-public).)

Layanan indeks kunci publik milik Vela yang mengirim catatan itu dan membayar gasnya;
catatannya sendiri tersimpan on-chain, dan aplikasi mana pun bisa membacanya langsung.

## Masuk di perangkat baru

1. Buka Vela dan pilih masuk.
2. Pakai **salah satu** kunci Anda: passkey yang sudah tersinkron ke perangkat ini,
   ponsel di dekat Anda (pindai kode QR), atau kunci keamanan Anda.
3. Vela mengetahui kunci publik dari kunci itu lewat tanda tangannya, lalu mencarinya —
   pertama di indeks Vela, lalu, kalau indeks tidak menjawab, di kontrak registri di
   Gnosis, kemudian di Ethereum — dan membangun ulang dompetnya. Sebelum menampilkannya
   kepada Anda, Vela memeriksa bahwa kunci-kunci yang ditemukannya memang menghasilkan
   alamat yang tercatat.

Daftar akun tidak disinkronkan antarperangkat; masuk akan membangunnya ulang.

<Callout type="info" title="Kalau tidak ada indeks atau registri yang menjawab">
Dompet dengan <strong>satu kunci</strong> bisa dibangun ulang di perangkat tanpa
melibatkan server sama sekali: dua tanda tangan dari kunci itu sudah cukup untuk
memulihkan kunci publiknya dan menghitung ulang alamatnya. Dompet dengan beberapa kunci
membutuhkan catatan registri, karena satu kunci tidak bisa memberi tahu aplikasi kunci
apa saja yang lainnya.
</Callout>

## Salinan catatannya

Registri di Gnosis adalah yang pertama dibaca aplikasi. Dari **Pengaturan**, Anda juga
bisa menyalin catatan dompet Anda ke kontrak registri yang sama di **Ethereum**, dengan
membayar gasnya sendiri, supaya catatan itu ada di chain kedua. Siapa pun bisa membuat
salinan seperti itu; isinya tidak ada yang bisa memindahkan dana.

## Batas yang jujur

<Callout type="warning" title="Kunci yang hilang, hilang">
Kalau setiap kunci yang Anda pakai saat membuat dompet sudah tidak ada — passkey yang
tersinkron, ponsel, kunci keamanan — tidak ada yang bisa memulihkan dompet itu, baik
Vela, Apple, Google, maupun siapa pun. Tidak ada frasa pemulihan, tidak ada
reset dari layanan pelanggan, dan tidak ada pintu belakang.
</Callout>

Yang membuat hal itu kecil kemungkinannya adalah punya lebih dari satu jalan masuk:

- **Biarkan sinkronisasi passkey tetap aktif** kalau Anda memakai passkey perangkat ini.
  Sinkronisasi itulah yang membawa kunci ke ponsel atau komputer baru.
- **Amankan akun di baliknya.** Siapa pun yang menguasai akun Apple atau Google Anda
  mungkin bisa memakai passkey yang tersinkron; beri akun itu kata sandi yang kuat dan
  opsi pemulihannya sendiri.
- **Buat dompet dengan lebih dari satu kunci**, misalnya passkey di ponsel Anda dan kunci
  keamanan fisik yang disimpan di tempat aman. Kunci hanya bisa ditambahkan saat Anda
  membuat dompet ([alasannya](/id/docs/signers)). Ingat bahwa satu kunci mana pun bisa
  menandatangani sendirian — dan tidak bisa dihapus, jadi kalau suatu saat ada kunci yang
  bocor, pindahkan dana Anda ke dompet baru ([caranya](/id/docs/signers)).

## Apa yang bisa dan tidak bisa dilakukan Vela

- **Bisa:** menjaga indeks tetap berjalan, supaya dompet Anda cepat ditemukan di
  perangkat baru.
- **Tidak bisa:** memindahkan dana Anda, membekukan dompet Anda, menambah atau menghapus
  kunci, atau memulihkan kunci yang sudah hilang. Vela tidak pernah memegang kunci Anda.

Berikutnya: [clear signing](/id/docs/clear-signing).
